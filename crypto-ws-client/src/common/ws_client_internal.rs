use std::{
    io::prelude::*,
    num::NonZeroU32,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
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
    // Добавляем handle для пинг-задачи, чтобы можно было отменить её при переподключении
    ping_task_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
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
                    ping_task_handle: std::sync::Mutex::new(None),
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

        // Специальная обработка для Binance - добавляем небольшие задержки между командами
        let delay =
            if self.exchange == "binance" { Some(Duration::from_millis(100)) } else { None };

        for command in commands {
            debug!("{}", command);
            if self.command_tx.send(Message::Text(command.to_string())).await.is_err() {
                break; // break the loop if there is no receiver
            }

            // Если это Binance, добавляем небольшую задержку между командами
            if let Some(d) = delay {
                tokio::time::sleep(d).await;
            }
        }
    }

    // Добавляем приватный метод для переподключения
    async fn reconnect(
        &self,
        _handler: H,
        _tx: std::sync::mpsc::Sender<String>,
    ) -> Option<tokio::sync::mpsc::Receiver<Message>> {
        // Устанавливаем флаг, что переподключение в процессе
        self.reconnect_in_progress.store(true, Ordering::SeqCst);

        // Отменяем старую пинг-задачу, если она существует
        {
            let mut guard = self.ping_task_handle.lock().unwrap();
            if let Some(handle) = guard.take() {
                info!("Aborting old ping task during reconnection");
                handle.abort();
            }
        }

        // Небольшая задержка перед первой попыткой переподключения
        // Это даст время отработать всем операциям отмены пинг-задачи
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Максимальное количество попыток переподключения
        const MAX_RECONNECT_ATTEMPTS: u32 = 5;
        // Начальная задержка в секундах
        let mut backoff_time = 2;

        // Для Binance используем специальный режим переподключения с большими интервалами
        let is_binance = self.exchange == "binance";
        if is_binance {
            backoff_time = 5; // Увеличиваем начальное время ожидания для Binance
        }

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

                        // Для Binance добавляем больший интервал между восстановлением подписок
                        let delay = if is_binance {
                            Duration::from_millis(300)
                        } else {
                            Duration::from_millis(100)
                        };

                        for command in &subscriptions {
                            debug!("Restoring subscription: {}", command);
                            if let Err(err) =
                                self.command_tx.send(Message::Text(command.clone())).await
                            {
                                error!("Failed to restore subscription: {}", err);
                            }
                            // Задержка между подписками
                            tokio::time::sleep(delay).await;
                        }
                    }

                    // Запускаем новую пинг-задачу после успешного переподключения
                    let num_unanswered_ping = Arc::new(AtomicIsize::new(0));
                    self.start_ping_task(&_handler, num_unanswered_ping);

                    self.reconnect_in_progress.store(false, Ordering::SeqCst);
                    return Some(message_rx);
                }
                Err(err) => {
                    error!(
                        "Failed to reconnect to {} (attempt {}/{}): {}",
                        self.url, attempt, MAX_RECONNECT_ATTEMPTS, err
                    );

                    // Экспоненциальное увеличение задержки (с ограничением)
                    let max_backoff = if is_binance { 120 } else { 60 }; // Для Binance увеличиваем максимальную задержку
                    backoff_time = std::cmp::min(backoff_time * 2, max_backoff);
                }
            }
        }

        error!(
            "Failed to reconnect to {} after {} attempts, giving up",
            self.url, MAX_RECONNECT_ATTEMPTS
        );
        self.reconnect_in_progress.store(false, Ordering::SeqCst);
        None
    }

    // Добавляем метод для запуска пинг-задачи
    fn start_ping_task(&self, handler: &H, num_unanswered_ping: Arc<AtomicIsize>) {
        if let Some((msg, interval)) = handler.get_ping_msg_and_interval() {
            // Создаем канал для отслеживания состояния соединения
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

            // send heartbeat periodically
            let command_tx_clone = self.command_tx.clone();
            let num_unanswered_ping_clone = num_unanswered_ping.clone();

            // Добавляем механизм проверки состояния соединения
            let url_clone = self.url.clone();
            let exchange_clone = self.exchange;
            // Используем Arc для безопасного доступа к AtomicBool между потоками
            let reconnect_in_progress_clone = self.reconnect_in_progress.clone();

            let ping_task = tokio::task::spawn(async move {
                let mut timer = {
                    let duration = Duration::from_secs(interval / 2 + 1);
                    tokio::time::interval(duration)
                };

                // Таймер для проверки состояния соединения
                let mut health_check_timer =
                    tokio::time::interval(Duration::from_secs(interval * 2));

                // Специальный обработчик для Binance - более короткий интервал проверки
                let is_binance = exchange_clone == "binance";
                let mut binance_check_timer = if is_binance {
                    tokio::time::interval(Duration::from_secs(30))
                } else {
                    // Для других бирж используем тот же интервал, но результат игнорируем
                    tokio::time::interval(Duration::from_secs(60))
                };

                loop {
                    tokio::select! {
                        now = timer.tick() => {
                            debug!("{:?} sending ping {}", now, msg.to_text().unwrap());
                            if let Err(err) = command_tx_clone.send(msg.clone()).await {
                                error!("Error sending ping {}", err);
                                // Если канал закрыт, выходим из цикла
                                break;
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
                                    // Если канал закрыт, выходим из цикла
                                    break;
                                } else {
                                    info!("Sent close message to initiate reconnection for {}", exchange_clone);
                                }
                            }
                        }

                        _ = binance_check_timer.tick() => {
                            // Проверка только для Binance
                            if is_binance {
                                let unanswered = num_unanswered_ping_clone.load(Ordering::Acquire);
                                if unanswered > 1 && !reconnect_in_progress_clone.load(Ordering::Acquire) {
                                    warn!(
                                        "Binance connection health check: {} unanswered pings for {}",
                                        unanswered, url_clone
                                    );

                                    // Для Binance отправляем Pong вместо Close для проверки соединения
                                    if let Err(err) = command_tx_clone.send(Message::Pong(Vec::new())).await {
                                        error!("Failed to send pong message to Binance: {}", err);
                                        break;
                                    } else {
                                        debug!("Sent pong message to Binance to check connection");
                                    }
                                }
                            }
                        }

                        // Проверяем сигнал остановки
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                info!("Ping task for {} received shutdown signal, stopping", exchange_clone);
                                break;
                            }
                        }
                    }
                }

                info!("Ping task for {} stopped", exchange_clone);
            });

            // Сохраняем handle пинг-задачи и sender для отмены
            {
                let mut guard = self.ping_task_handle.lock().unwrap();
                *guard = Some(ping_task);
            }

            // Добавляем инициализирующий Pong сразу после создания пинг-задачи для Binance
            // Это помогает установить корректное соединение сразу, особенно для user_data каналов
            if self.exchange == "binance" {
                let cmd_tx = self.command_tx.clone();
                tokio::spawn(async move {
                    // Даем немного времени на установление соединения
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    debug!("Sending initial pong to Binance after connection setup");
                    // Отправляем пустой Pong фрейм для инициализации соединения
                    if let Err(err) = cmd_tx.send(Message::Pong(Vec::new())).await {
                        warn!("Failed to send initial pong to Binance: {}", err);
                    }
                });
            }

            // Создаем отдельную задачу для мониторинга состояния переподключения
            let reconnect_in_progress_clone = self.reconnect_in_progress.clone();
            let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));

            // Запускаем задачу, которая будет следить за флагом reconnect_in_progress
            // и отправлять сигнал остановки пинг-задаче при необходимости
            tokio::task::spawn(async move {
                let mut check_interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    check_interval.tick().await;

                    // Если началось переподключение, отправляем сигнал остановки
                    if reconnect_in_progress_clone.load(Ordering::Acquire) {
                        let mut guard = shutdown_tx.lock().unwrap();
                        if let Some(tx) = guard.take() {
                            info!(
                                "Sending shutdown signal to ping task for {} due to reconnection",
                                exchange_clone
                            );
                            if let Err(err) = tx.send(true) {
                                error!("Failed to send shutdown signal to ping task: {}", err);
                            }
                            // Выходим из цикла после отправки сигнала
                            break;
                        } else {
                            // Сигнал уже был отправлен или канал закрыт, выходим
                            break;
                        }
                    }
                }
            });
        }
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
        self.start_ping_task(&handler, num_unanswered_ping.clone());

        // Для Binance добавляем дополнительную диагностику
        let is_binance = self.exchange == "binance";
        if is_binance {
            info!("Starting WebSocket connection for Binance with special handling");
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
                            std::str::from_utf8(&resp).unwrap_or("non-utf8"),
                            self.url,
                        );
                        if self.exchange == "binance" {
                            // send a pong frame
                            debug!("Sending a pong frame to {}", self.url);
                            // Для Binance обязательно отправляем пустой Pong фрейм
                            // и сбрасываем счетчик неотвеченных пингов
                            if let Err(err) = self.command_tx.send(Message::Pong(Vec::new())).await
                            {
                                error!("Failed to send pong response to Binance: {}", err);
                                // Если не можем отправить pong, соединение возможно мертво
                                warn!("Could not send pong to Binance, connection might be dead");
                                break; // Выходим из цикла, чтобы вызвать переподключение
                            } else {
                                // Явно обнуляем счетчик при успешной отправке Pong
                                num_unanswered_ping.store(0, Ordering::Release);
                                debug!(
                                    "Successfully sent pong response to Binance ping, reset unanswered count to 0"
                                );
                            }
                        }
                        None
                    }
                    Message::Pong(resp) => {
                        num_unanswered_ping.store(0, Ordering::Release);
                        debug!(
                            "Received a pong frame: {} from {}, reset num_unanswered_ping to {}",
                            std::str::from_utf8(&resp).unwrap_or("non-utf8"),
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

                        // Отменяем пинг-задачу здесь, перед выходом из цикла сообщений
                        {
                            let mut guard = self.ping_task_handle.lock().unwrap();
                            if let Some(handle) = guard.take() {
                                info!("Aborting ping task due to connection close");
                                handle.abort();
                            }
                        }

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
                        MiscMessage::Reconnect => {
                            info!("Received explicit reconnect request from message handler");
                            break; // Выходим из внутреннего цикла для переподключения
                        }
                        MiscMessage::Other => (), // ignore
                    }
                }
            }

            // Проверяем, была ли закрыта очередь сообщений
            if !self.reconnect_in_progress.load(Ordering::SeqCst) {
                info!("Message queue closed, attempting to reconnect...");

                // Создаем новый клон handler для переподключения
                let handler_clone_for_reconnect =
                    unsafe { std::ptr::read(&handler_clone as *const H) };

                // Пытаемся переподключиться
                if let Some(new_message_rx) =
                    self.reconnect(handler_clone_for_reconnect, tx.clone()).await
                {
                    // Если переподключение успешно, обновляем message_rx и продолжаем цикл
                    message_rx = new_message_rx;

                    if is_binance {
                        info!("Successfully reconnected to Binance, continuing operations");
                    }

                    continue 'connection_loop;
                } else {
                    // Если переподключение не удалось после нескольких попыток, выходим
                    error!("Failed to reconnect after multiple attempts, exiting...");
                    break 'connection_loop;
                }
            } else {
                // Если переподключение уже в процессе, ждем немного и выходим
                warn!("Reconnection already in progress, waiting before exiting...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                break 'connection_loop;
            }
        }

        info!("WebSocket client for {} has stopped", self.exchange);
    }

    pub async fn close(&self) {
        // Отменяем пинг-задачу при закрытии соединения
        {
            let mut guard = self.ping_task_handle.lock().unwrap();
            if let Some(handle) = guard.take() {
                info!("Aborting ping task during connection close");
                handle.abort();
            }
        }

        // close the websocket connection and break the while loop in run()
        _ = self.command_tx.send(Message::Close(None)).await;
    }
}
