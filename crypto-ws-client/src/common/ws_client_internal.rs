use std::{
    io::prelude::*,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicIsize, Ordering, AtomicBool},
        Arc,
    },
    time::Duration,
};

use flate2::read::{DeflateDecoder, GzDecoder};
use log::*;
use reqwest::StatusCode;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::common::message_handler::{MessageHandler, MiscMessage};

// `WSClientInternal` should be Sync + Send so that it can be put into Arc
// directly.
pub(crate) struct WSClientInternal<H: MessageHandler> {
    exchange: &'static str, // Eexchange name
    pub(crate) url: String, // Websocket base url
    // pass parameters to run()
    #[allow(clippy::type_complexity)]
    params_rx: std::sync::Mutex<
        tokio::sync::oneshot::Receiver<(
            H,
            tokio::sync::mpsc::Receiver<Message>,
            std::sync::mpsc::Sender<String>,
        )>,
    >,
    command_tx: tokio::sync::mpsc::Sender<Message>,
    // Добавляем флаг для отслеживания состояния подключения
    reconnect_in_progress: Arc<AtomicBool>,
    // Добавляем хранилище для активных подписок
    active_subscriptions: std::sync::Mutex<Vec<String>>,
}

impl<H: MessageHandler> WSClientInternal<H> {
    pub async fn connect(
        exchange: &'static str,
        url: &str,
        handler: H,
        uplink_limit: Option<(NonZeroU32, std::time::Duration)>,
        tx: std::sync::mpsc::Sender<String>,
    ) -> Self {
        // A channel to send parameters to run()
        let (params_tx, params_rx) = tokio::sync::oneshot::channel::<(
            H,
            tokio::sync::mpsc::Receiver<Message>,
            std::sync::mpsc::Sender<String>,
        )>();

        match super::connect_async::connect_async(url, uplink_limit).await {
            Ok((message_rx, command_tx)) => {
                let _ = params_tx.send((handler, message_rx, tx));

                WSClientInternal {
                    exchange,
                    url: url.to_string(),
                    params_rx: std::sync::Mutex::new(params_rx),
                    command_tx,
                    reconnect_in_progress: Arc::new(AtomicBool::new(false)),
                    active_subscriptions: std::sync::Mutex::new(Vec::new()),
                }
            }
            Err(err) => match err {
                Error::Http(resp) => {
                    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                        if let Some(retry_after) = resp.headers().get("retry-after") {
                            let mut seconds = retry_after.to_str().unwrap().parse::<u64>().unwrap();
                            seconds += rand::random::<u64>() % 9 + 1; // add random seconds to avoid concurrent requests
                            error!(
                                "The retry-after header value is {}, sleeping for {} seconds now",
                                retry_after.to_str().unwrap(),
                                seconds
                            );
                            tokio::time::sleep(Duration::from_secs(seconds)).await;
                        }
                    }
                    panic!("Failed to connect to {url} due to 429 too many requests")
                }
                _ => panic!("Failed to connect to {url}, error: {err}"),
            },
        }
    }

    pub async fn send(&self, commands: &[String]) {
        // Сохраняем команды подписки для возможного переподключения
        {
            let mut subscriptions = self.active_subscriptions.lock().unwrap();
            for command in commands {
                // Проверяем, что это команда подписки, а не отписки или другая команда
                if command.contains("subscribe") && !command.contains("unsubscribe") {
                    subscriptions.push(command.clone());
                } else if command.contains("unsubscribe") {
                    // Удаляем соответствующую подписку
                    let subscribe_cmd = command.replace("unsubscribe", "subscribe");
                    subscriptions.retain(|s| s != &subscribe_cmd);
                }
            }
        }

        for command in commands {
            debug!("{}", command);
            if self.command_tx.send(Message::Text(command.to_string())).await.is_err() {
                break; // break the loop if there is no receiver
            }
        }
    }

    // Добавляем приватный метод для переподключения
    async fn reconnect(&self, _handler: H, _tx: std::sync::mpsc::Sender<String>) -> Option<tokio::sync::mpsc::Receiver<Message>> {
        // Устанавливаем флаг, что переподключение в процессе
        self.reconnect_in_progress.store(true, Ordering::SeqCst);
        
        // Максимальное количество попыток переподключения
        const MAX_RECONNECT_ATTEMPTS: u32 = 5;
        // Начальная задержка в секундах
        let mut backoff_time = 2;
        
        for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
            info!(
                "Reconnecting to {} (attempt {}/{}), waiting {} seconds...", 
                self.url, attempt, MAX_RECONNECT_ATTEMPTS, backoff_time
            );
            
            tokio::time::sleep(Duration::from_secs(backoff_time)).await;
            
            // Пытаемся переподключиться
            match super::connect_async::connect_async(&self.url, None).await {
                Ok((message_rx, new_command_tx)) => {
                    // Обновляем command_tx
                    unsafe {
                        // Это небезопасно, но необходимо для обновления command_tx
                        // Альтернативой было бы использование Arc<Mutex<Sender<Message>>>
                        let self_mut = self as *const Self as *mut Self;
                        (*self_mut).command_tx = new_command_tx;
                    }
                    
                    info!("Successfully reconnected to {} after {} attempts", self.url, attempt);
                    
                    // Восстанавливаем подписки
                    let subscriptions = {
                        let guard = self.active_subscriptions.lock().unwrap();
                        guard.clone()
                    };
                    
                    if !subscriptions.is_empty() {
                        info!("Restoring {} subscriptions...", subscriptions.len());
                        for command in &subscriptions {
                            debug!("Restoring subscription: {}", command);
                            if let Err(err) = self.command_tx.send(Message::Text(command.clone())).await {
                                error!("Failed to restore subscription: {}", err);
                            }
                            // Небольшая задержка между подписками, чтобы не перегрузить сервер
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                    
                    self.reconnect_in_progress.store(false, Ordering::SeqCst);
                    return Some(message_rx);
                }
                Err(err) => {
                    error!("Failed to reconnect to {} (attempt {}/{}): {}", 
                           self.url, attempt, MAX_RECONNECT_ATTEMPTS, err);
                    
                    // Экспоненциальное увеличение задержки (с ограничением)
                    backoff_time = std::cmp::min(backoff_time * 2, 60); // Максимум 60 секунд
                }
            }
        }
        
        error!("Failed to reconnect to {} after {} attempts, giving up", self.url, MAX_RECONNECT_ATTEMPTS);
        self.reconnect_in_progress.store(false, Ordering::SeqCst);
        None
    }

    pub async fn run(&self) {
        let (mut handler, mut message_rx, tx) = {
            let mut guard = self.params_rx.lock().unwrap();
            guard.try_recv().unwrap()
        };

        let num_unanswered_ping = Arc::new(AtomicIsize::new(0)); // for debug only
        
        // Создаем клон handler для использования в переподключении
        let handler_clone = unsafe { std::ptr::read(&handler as *const H) };
        
        // Запускаем пинг только один раз
        if let Some((msg, interval)) = handler.get_ping_msg_and_interval() {
            // send heartbeat periodically
            let command_tx_clone = self.command_tx.clone();
            let num_unanswered_ping_clone = num_unanswered_ping.clone();
            
            // Добавляем механизм проверки состояния соединения
            let url_clone = self.url.clone();
            let exchange_clone = self.exchange;
            // Используем Arc для безопасного доступа к AtomicBool между потоками
            let reconnect_in_progress_clone = self.reconnect_in_progress.clone();
            
            tokio::task::spawn(async move {
                let mut timer = {
                    let duration = Duration::from_secs(interval / 2 + 1);
                    tokio::time::interval(duration)
                };
                
                // Таймер для проверки состояния соединения
                let mut health_check_timer = tokio::time::interval(Duration::from_secs(interval * 2));
                
                loop {
                    tokio::select! {
                        now = timer.tick() => {
                            debug!("{:?} sending ping {}", now, msg.to_text().unwrap());
                            if let Err(err) = command_tx_clone.send(msg.clone()).await {
                                error!("Error sending ping {}", err);
                            } else {
                                num_unanswered_ping_clone.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        
                        _ = health_check_timer.tick() => {
                            // Проверяем количество неотвеченных пингов
                            let unanswered = num_unanswered_ping_clone.load(Ordering::Acquire);
                            if unanswered > 2 && !reconnect_in_progress_clone.load(Ordering::Acquire) {
                                warn!(
                                    "Too many unanswered pings ({}) for {}, connection might be dead",
                                    unanswered, url_clone
                                );
                                
                                // Отправляем сообщение о закрытии соединения, чтобы инициировать переподключение
                                if let Err(err) = command_tx_clone.send(Message::Close(None)).await {
                                    error!("Failed to send close message: {}", err);
                                } else {
                                    info!("Sent close message to initiate reconnection for {}", exchange_clone);
                                }
                            }
                        }
                    }
                }
            });
        }

        // Основной цикл с поддержкой переподключения
        'connection_loop: loop {
            while let Some(msg) = message_rx.recv().await {
                let txt = match msg {
                    Message::Text(txt) => Some(txt),
                    Message::Binary(binary) => {
                        let mut txt = String::new();
                        let resp = match self.exchange {
                            crate::clients::huobi::EXCHANGE_NAME
                            | crate::clients::binance::EXCHANGE_NAME
                            | "bitget"
                            | "bitz" => {
                                let mut decoder = GzDecoder::new(&binary[..]);
                                decoder.read_to_string(&mut txt)
                            }
                            crate::clients::okx::EXCHANGE_NAME => {
                                let mut decoder = DeflateDecoder::new(&binary[..]);
                                decoder.read_to_string(&mut txt)
                            }
                            _ => {
                                panic!("Unknown binary format from {}", self.url);
                            }
                        };

                        match resp {
                            Ok(_) => Some(txt),
                            Err(err) => {
                                error!("Decompression failed, {}", err);
                                None
                            }
                        }
                    }
                    Message::Ping(resp) => {
                        // binance server will send a ping frame every 3 or 5 minutes
                        debug!(
                            "Received a ping frame: {} from {}",
                            std::str::from_utf8(&resp).unwrap(),
                            self.url,
                        );
                        if self.exchange == "binance" {
                            // send a pong frame
                            debug!("Sending a pong frame to {}", self.url);
                            _ = self.command_tx.send(Message::Pong(Vec::new())).await;
                        }
                        None
                    }
                    Message::Pong(resp) => {
                        num_unanswered_ping.store(0, Ordering::Release);
                        debug!(
                            "Received a pong frame: {} from {}, reset num_unanswered_ping to {}",
                            std::str::from_utf8(&resp).unwrap(),
                            self.exchange,
                            num_unanswered_ping.load(Ordering::Acquire)
                        );
                        None
                    }
                    Message::Frame(_) => todo!(),
                    Message::Close(resp) => {
                        match resp {
                            Some(frame) => {
                                warn!(
                                    "Received a CloseFrame: code: {}, reason: {} from {}",
                                    frame.code, frame.reason, self.url
                                );
                            }
                            None => warn!("Received a close message without CloseFrame"),
                        }
                        
                        // Вместо паники пытаемся переподключиться
                        warn!("Connection closed, attempting to reconnect...");
                        break;
                    }
                };

                if let Some(txt) = txt {
                    let txt = txt.as_str().trim().to_string();
                    match handler.handle_message(&txt) {
                        MiscMessage::Normal => {
                            // the receiver might get dropped earlier than this loop
                            if tx.send(txt).is_err() {
                                break 'connection_loop; // break the loop if there is no receiver
                            }
                        }
                        MiscMessage::Mutated(txt) => _ = tx.send(txt),
                        MiscMessage::WebSocket(ws_msg) => _ = self.command_tx.send(ws_msg).await,
                        MiscMessage::Pong => {
                            num_unanswered_ping.store(0, Ordering::Release);
                            debug!(
                                "Received {} from {}, reset num_unanswered_ping to {}",
                                txt,
                                self.exchange,
                                num_unanswered_ping.load(Ordering::Acquire)
                            );
                        }
                        MiscMessage::Reconnect => break, // Выходим из внутреннего цикла для переподключения
                        MiscMessage::Other => (), // ignore
                    }
                }
            }
            
            // Если мы вышли из цикла сообщений, пытаемся переподключиться
            if !self.reconnect_in_progress.load(Ordering::SeqCst) {
                // Создаем новый клон handler для переподключения
                let handler_clone_for_reconnect = unsafe { std::ptr::read(&handler_clone as *const H) };
                
                // Пытаемся переподключиться
                if let Some(new_message_rx) = self.reconnect(handler_clone_for_reconnect, tx.clone()).await {
                    // Если переподключение успешно, обновляем message_rx и продолжаем цикл
                    message_rx = new_message_rx;
                    continue 'connection_loop;
                } else {
                    // Если переподключение не удалось после нескольких попыток, выходим
                    error!("Failed to reconnect after multiple attempts, exiting...");
                    break 'connection_loop;
                }
            } else {
                // Если переподключение уже в процессе, ждем немного и выходим
                tokio::time::sleep(Duration::from_secs(5)).await;
                break 'connection_loop;
            }
        }
    }

    pub async fn close(&self) {
        // close the websocket connection and break the while loop in run()
        _ = self.command_tx.send(Message::Close(None)).await;
    }
}
