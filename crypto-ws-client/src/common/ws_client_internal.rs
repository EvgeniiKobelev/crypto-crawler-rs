use std::{
    io::prelude::*,
    num::NonZeroU32,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use flate2::read::{DeflateDecoder, GzDecoder};
use log::*;
use rand;
use reqwest::StatusCode;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::common::message_handler::{MessageHandler, MiscMessage};

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

#[derive(Debug, Default)]
pub struct ConnectionMetrics {
    pub total_connections: AtomicU64,
    pub successful_connections: AtomicU64,
    pub failed_connections: AtomicU64,
    pub reconnection_attempts: AtomicU64,
    pub ping_failures: AtomicU64,
    pub last_error: Mutex<Option<String>>,
}

impl ConnectionMetrics {
    pub fn record_connection_attempt(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_connection_success(&self) {
        self.successful_connections.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_connection_failure(&self, error: &str) {
        self.failed_connections.fetch_add(1, Ordering::Relaxed);
        *self.last_error.lock().unwrap() = Some(error.to_string());
    }
    
    pub fn record_reconnection_attempt(&self) {
        self.reconnection_attempts.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_ping_failure(&self) {
        self.ping_failures.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct HealthStatus {
    pub state: ConnectionState,
    pub total_connections: u64,
    pub successful_connections: u64,
    pub failed_connections: u64,
    pub reconnection_attempts: u64,
    pub ping_failures: u64,
    pub last_ping: i64,
    pub uptime: Duration,
    pub last_error: Option<String>,
}

fn log_connection_event(exchange: &str, event: &str, details: &str) {
    info!(
        target: "websocket_connection",
        "WebSocket connection event - exchange: {}, event: {}, details: {}, timestamp: {}",
        exchange, event, details, chrono::Utc::now().to_rfc3339()
    );
}

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
    // Новые поля для улучшенного управления состоянием
    connection_state: Mutex<ConnectionState>,
    metrics: ConnectionMetrics,
    start_time: Instant,
    last_ping_time: AtomicU64,
    ping_shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

impl<H: MessageHandler> WSClientInternal<H> {
    fn set_connection_state(&self, state: ConnectionState) {
        let mut guard = self.connection_state.lock().unwrap();
        if *guard != state {
            log_connection_event(self.exchange, "state_change", &format!("{:?} -> {:?}", *guard, state));
            *guard = state;
        }
    }
    
    pub fn get_health_status(&self) -> HealthStatus {
        let state = self.connection_state.lock().unwrap().clone();
        let last_error = self.metrics.last_error.lock().unwrap().clone();
        
        HealthStatus {
            state,
            total_connections: self.metrics.total_connections.load(Ordering::Relaxed),
            successful_connections: self.metrics.successful_connections.load(Ordering::Relaxed),
            failed_connections: self.metrics.failed_connections.load(Ordering::Relaxed),
            reconnection_attempts: self.metrics.reconnection_attempts.load(Ordering::Relaxed),
            ping_failures: self.metrics.ping_failures.load(Ordering::Relaxed),
            last_ping: self.last_ping_time.load(Ordering::Relaxed) as i64,
            uptime: self.start_time.elapsed(),
            last_error,
        }
    }
    
    fn stop_ping_task_safely(&self) {
        let mut guard = self.ping_task_handle.lock().unwrap();
        if let Some(handle) = guard.take() {
            // Сначала пытаемся graceful shutdown
            if let Some(shutdown_tx) = self.ping_shutdown_tx.lock().unwrap().take() {
                match shutdown_tx.send(true) {
                    Ok(_) => {
                        debug!("Sent graceful shutdown signal to ping task for {}", self.exchange);
                        // Даем время на graceful shutdown
                        let handle_clone = handle;
                        tokio::spawn(async move {
                            match tokio::time::timeout(Duration::from_secs(2), handle_clone).await {
                                Ok(_) => debug!("Ping task shutdown gracefully"),
                                Err(_) => {
                                    warn!("Ping task didn't shutdown gracefully within timeout");
                                }
                            }
                        });
                    }
                    Err(_) => {
                        debug!("Ping task channel already closed for {}, aborting task", self.exchange);
                        handle.abort();
                    }
                }
            } else {
                // Нет канала - просто abort
                debug!("No shutdown channel available for {}, aborting ping task", self.exchange);
                handle.abort();
            }
        }
    }
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

        // Максимальное количество попыток подключения
        const MAX_CONNECTION_ATTEMPTS: u32 = 5;
        let mut backoff_time = 2; // Начальная задержка в секундах

        // Для MEXC используем более длительные интервалы из-за строгих лимитов
        let is_mexc = exchange == "mexc";
        if is_mexc {
            backoff_time = 5;
        }

        for attempt in 1..=MAX_CONNECTION_ATTEMPTS {
            log_connection_event(exchange, "connection_attempt", &format!("Attempt {}/{}", attempt, MAX_CONNECTION_ATTEMPTS));
            
            match super::connect_async::connect_async(url, uplink_limit).await {
                Ok((message_rx, command_tx)) => {
                    let _ = params_tx.send((handler, message_rx, tx));
                    
                    log_connection_event(exchange, "connection_success", "WebSocket connected successfully");

                    return WSClientInternal {
                        exchange,
                        url: url.to_string(),
                        params_rx: std::sync::Mutex::new(params_rx),
                        command_tx,
                        reconnect_in_progress: Arc::new(AtomicBool::new(false)),
                        active_subscriptions: std::sync::Mutex::new(Vec::new()),
                        ping_task_handle: std::sync::Mutex::new(None),
                        connection_state: Mutex::new(ConnectionState::Connected),
                        metrics: ConnectionMetrics::default(),
                        start_time: Instant::now(),
                        last_ping_time: AtomicU64::new(chrono::Utc::now().timestamp() as u64),
                        ping_shutdown_tx: Mutex::new(None),
                    };
                }
                Err(err) => match err {
                    Error::Http(resp) => {
                        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                            let retry_seconds =
                                if let Some(retry_after) = resp.headers().get("retry-after") {
                                    let mut seconds = retry_after
                                        .to_str()
                                        .unwrap_or("60")
                                        .parse::<u64>()
                                        .unwrap_or(60);
                                    seconds += rand::random::<u64>() % 9 + 1; // add random seconds to avoid concurrent requests
                                    seconds
                                } else {
                                    // Если нет retry-after заголовка, используем экспоненциальный backoff
                                    backoff_time + (rand::random::<u64>() % 10)
                                };

                            if attempt < MAX_CONNECTION_ATTEMPTS {
                                warn!(
                                    "Failed to connect to {} due to 429 too many requests (attempt {}/{}), waiting {} seconds before retry",
                                    url, attempt, MAX_CONNECTION_ATTEMPTS, retry_seconds
                                );
                                tokio::time::sleep(Duration::from_secs(retry_seconds)).await;

                                // Увеличиваем время ожидания для следующей попытки
                                let max_backoff = if is_mexc { 300 } else { 120 }; // Для MEXC используем более длительный максимум
                                backoff_time = std::cmp::min(backoff_time * 2, max_backoff);
                                continue;
                            } else {
                                error!(
                                    "Failed to connect to {} due to 429 too many requests after {} attempts, giving up",
                                    url, MAX_CONNECTION_ATTEMPTS
                                );
                                panic!(
                                    "Failed to connect to {url} due to 429 too many requests after {MAX_CONNECTION_ATTEMPTS} attempts"
                                )
                            }
                        } else {
                            panic!(
                                "Failed to connect to {url} due to HTTP error: {}",
                                resp.status()
                            )
                        }
                    }
                    _ => {
                        // Специальная диагностика для MEXC User Data Stream
                        if is_mexc && url.contains("wbs-api.mexc.com") && url.contains("listenKey=")
                        {
                            error!("MEXC User Data Stream подключение отклонено сервером");
                            error!("Возможные причины:");
                            error!("1. Неправильный или истёкший listen_key");
                            error!("2. Listen key был получен для другого API аккаунта");
                            error!("3. Listen key уже использован в другом подключении");
                            error!("4. API ключ не имеет прав на создание User Data Stream");
                            error!("Создайте новый listen_key через REST API:");
                            error!(
                                "curl -X POST \"https://api.mexc.com/api/v3/userDataStream\" -H \"X-MEXC-APIKEY: your_api_key\""
                            );
                        }

                        if attempt < MAX_CONNECTION_ATTEMPTS {
                            warn!(
                                "Failed to connect to {} (attempt {}/{}): {}, retrying...",
                                url, attempt, MAX_CONNECTION_ATTEMPTS, err
                            );
                            tokio::time::sleep(Duration::from_secs(backoff_time)).await;
                            backoff_time = std::cmp::min(backoff_time * 2, 60);
                            continue;
                        } else {
                            if is_mexc && url.contains("wbs-api.mexc.com") {
                                error!(
                                    "Не удалось подключиться к MEXC User Data Stream после {} попыток",
                                    MAX_CONNECTION_ATTEMPTS
                                );
                                error!("Убедитесь, что listen_key правильный и актуальный");
                                panic!("MEXC User Data Stream connection failed: {err}")
                            } else {
                                panic!("Failed to connect to {url}, error: {err}")
                            }
                        }
                    }
                },
            }
        }

        // Этот код никогда не должен быть достигнут, но добавляем для полноты
        panic!("Failed to connect to {url} after {MAX_CONNECTION_ATTEMPTS} attempts")
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
                    self.metrics.record_reconnection_attempt();
                    
                    match super::connect_async::connect_async(&self.url, None).await {
                        Ok((message_rx, new_command_tx)) => {
                            // Обновляем command_tx
                            unsafe {
                                // Это небезопасно, но необходимо для обновления command_tx
                                // Альтернативой было бы использование Arc<Mutex<Sender<Message>>>
                                let self_mut = self as *const Self as *mut Self;
                                (*self_mut).command_tx = new_command_tx;
                            }

                            self.metrics.record_connection_success();
                            self.set_connection_state(ConnectionState::Connected);
                            log_connection_event(self.exchange, "reconnection_success", &format!("Reconnected after {} attempts", attempt));

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
                    self.metrics.record_connection_failure(&err.to_string());
                    log_connection_event(self.exchange, "reconnection_failed", &format!("Attempt {}: {}", attempt, err));
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

                self.set_connection_state(ConnectionState::Failed("Max reconnection attempts exceeded".to_string()));
                log_connection_event(self.exchange, "reconnection_failed_final", &format!("Giving up after {} attempts", MAX_RECONNECT_ATTEMPTS));
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
            
            // Сохраняем sender для graceful shutdown
            *self.ping_shutdown_tx.lock().unwrap() = Some(shutdown_tx.clone());

            // send heartbeat periodically
            let command_tx_clone = self.command_tx.clone();
            let num_unanswered_ping_clone = num_unanswered_ping.clone();

            // Добавляем механизм проверки состояния соединения
            let url_clone = self.url.clone();
            let exchange_clone = self.exchange;
            // Переменная для будущего использования в мониторинге
            let _reconnect_in_progress_clone = self.reconnect_in_progress.clone();

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
                                error!("Error sending ping to {}: {}", exchange_clone, err);
                                // Записываем метрику ошибки ping
                                log_connection_event(exchange_clone, "ping_failure", &format!("Failed to send ping: {}", err));
                                // Если канал закрыт, выходим из цикла
                                break;
                            } else {
                                num_unanswered_ping_clone.fetch_add(1, Ordering::SeqCst);
                                // Обновляем время последнего ping
                                // last_ping_time.store(chrono::Utc::now().timestamp() as u64, Ordering::Relaxed);
                                debug!("Ping sent successfully to {}", exchange_clone);
                            }
                        }

                        _ = health_check_timer.tick() => {
                            // Проверяем количество неотвеченных пингов
                            let unanswered = num_unanswered_ping_clone.load(Ordering::Acquire);
                            if unanswered > 2 && !_reconnect_in_progress_clone.load(Ordering::Acquire) {
                                warn!(
                                    "Too many unanswered pings ({}) for {}, connection might be dead",
                                    unanswered, url_clone
                                );
                                
                                log_connection_event(exchange_clone, "ping_timeout", &format!("Too many unanswered pings: {}", unanswered));

                                // Отправляем сообщение о закрытии соединения, чтобы инициировать переподключение
                                if let Err(err) = command_tx_clone.send(Message::Close(None)).await {
                                    error!("Failed to send close message to {}: {}", exchange_clone, err);
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
                                if unanswered > 1 && !_reconnect_in_progress_clone.load(Ordering::Acquire) {
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
        }
    }

    pub async fn run(&self) {
        let (mut handler, mut message_rx, tx) = {
            let mut guard = self.params_rx.lock().unwrap();
            match guard.try_recv() {
                Ok(params) => params,
                Err(err) => {
                    error!(
                        "Не удалось получить параметры WebSocket соединения для {}: {:?}",
                        self.exchange, err
                    );
                    error!("Возможно, соединение было закрыто до инициализации");
                    return;
                }
            }
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
                            crate::clients::mexc::EXCHANGE_NAME => {
                                // MEXC может использовать разные форматы, попробуем несколько вариантов
                                if binary.len() > 0 {
                                    // Попробуем определить формат данных по первым байтам
                                    debug!(
                                        "MEXC binary data - первые 10 байт: {:?}",
                                        &binary[..std::cmp::min(10, binary.len())]
                                    );

                                    // Проверяем типичные заголовки сжатия СНАЧАЛА
                                    let is_gzip =
                                        binary.len() >= 2 && binary[0] == 0x1f && binary[1] == 0x8b;
                                    let is_deflate_zlib = binary.len() >= 2
                                        && binary[0] == 0x78
                                        && (binary[1] == 0x01 || binary[1] == 0x9c || binary[1] == 0xda);

                                    // Улучшенное определение Protocol Buffers
                                    // Protobuf часто начинается с varint field number + wire type
                                    // Первые байты [10, 30] = field 1, wire type 2 (length-delimited), length 30
                                    let is_likely_protobuf = binary.len() >= 4 && 
                                        !is_gzip && !is_deflate_zlib &&
                                        (
                                            // Типичные protobuf паттерны
                                            (binary[0] == 0x08 && binary[1] < 0x80) || // field 1, varint
                                            (binary[0] == 0x0a && binary[1] < 0x80) || // field 1, length-delimited
                                            (binary[0] == 0x10 && binary[1] < 0x80) || // field 2, varint
                                            (binary[0] == 0x12 && binary[1] < 0x80) || // field 2, length-delimited
                                            // Специально для данного случая: [10, 30, "spot@private..."]
                                            (binary[0] == 0x0a && binary.len() > 10 && 
                                             binary[2..].starts_with(b"spot@"))
                                        );

                                    debug!(
                                        "MEXC binary analysis: is_gzip={}, is_deflate_zlib={}, is_likely_protobuf={}",
                                        is_gzip, is_deflate_zlib, is_likely_protobuf
                                    );

                                    if is_likely_protobuf {
                                        // Определенно Protocol Buffers данные
                                        info!("🔍 MEXC: Обнаружены Protocol Buffers данные (длина: {})", binary.len());
                                        
                                        // Попробуем декодировать protobuf данные
                                        match crate::clients::mexc::decode_mexc_protobuf(&binary) {
                                            Ok(json_string) => {
                                                info!("✅ Успешно декодированы protobuf данные в JSON");
                                                debug!("Декодированный JSON: {}", json_string);
                                                txt = json_string;
                                                Ok(txt.len())
                                            }
                                            Err(decode_err) => {
                                                // Если декодирование не удалось, показываем диагностику
                                                warn!("❌ Не удалось декодировать protobuf данные: {}", decode_err);
                                                
                                                // Попробуем извлечь информацию о канале из protobuf для диагностики
                                                if binary.len() > 10 && binary[0] == 0x0a {
                                                    let channel_length = binary[1] as usize;
                                                    if binary.len() > 2 + channel_length {
                                                        if let Ok(channel_name) = String::from_utf8(binary[2..2+channel_length].to_vec()) {
                                                            warn!("📡 Канал протобуф: '{}'", channel_name);
                                                            if channel_name.contains(".pb") {
                                                                warn!("💡 Совет: возможно используется другая protobuf схема");
                                                                warn!("   Рекомендация: используйте JSON канал '{}'", channel_name.replace(".pb", ""));
                                                            }
                                                        }
                                                    }
                                                }

                                                warn!("📖 См. README_mexc_websocket_troubleshooting.md для подробностей");

                                                Err(std::io::Error::new(
                                                    std::io::ErrorKind::InvalidData,
                                                    format!("Protocol Buffers decoding failed: {}", decode_err),
                                                ))
                                            }
                                        }
                                    } else if is_gzip {
                                        // Данные сжаты gzip
                                        debug!("Trying GZIP decompression for MEXC");
                                        let mut gzip_decoder = GzDecoder::new(&binary[..]);
                                        gzip_decoder.read_to_string(&mut txt)
                                    } else if is_deflate_zlib {
                                        // Данные сжаты deflate/zlib
                                        debug!("Trying DEFLATE decompression for MEXC");
                                        let mut deflate_decoder = DeflateDecoder::new(&binary[..]);
                                        deflate_decoder.read_to_string(&mut txt)
                                    } else {
                                        // Возможно это несжатые JSON данные
                                        debug!("Trying raw UTF-8 parsing for MEXC");
                                        match String::from_utf8(binary.clone()) {
                                            Ok(utf8_string) => {
                                                if utf8_string.trim().starts_with('{')
                                                    || utf8_string.trim().starts_with('[')
                                                {
                                                    // Это JSON данные
                                                    txt = utf8_string;
                                                    Ok(txt.len())
                                                } else {
                                                    // Не JSON и не protobuf - неизвестный формат
                                                    warn!("MEXC: Неизвестный формат данных (длина: {})", binary.len());
                                                    warn!("Первые 20 байт: {:?}", &binary[..std::cmp::min(20, binary.len())]);
                                                    
                                                    Err(std::io::Error::new(
                                                        std::io::ErrorKind::InvalidData,
                                                        "Unknown data format - not JSON, not protobuf, not compressed",
                                                    ))
                                                }
                                            }
                                            Err(utf8_error) => {
                                                // Не UTF-8, последняя попытка - raw deflate
                                                debug!("Trying raw DEFLATE decompression for MEXC");
                                                txt.clear();

                                                use flate2::read::DeflateDecoder;
                                                use std::io::Cursor;

                                                let cursor = Cursor::new(&binary);
                                                let mut raw_deflate_decoder = DeflateDecoder::new(cursor);
                                                match raw_deflate_decoder.read_to_string(&mut txt) {
                                                    Ok(_) => {
                                                        if !txt.is_empty()
                                                            && (txt.trim().starts_with('{')
                                                                || txt.trim().starts_with('['))
                                                        {
                                                            debug!("Successfully decompressed with raw DEFLATE");
                                                            Ok(txt.len())
                                                        } else {
                                                            Err(std::io::Error::new(
                                                                std::io::ErrorKind::InvalidData,
                                                                "Raw DEFLATE produced non-JSON content",
                                                            ))
                                                        }
                                                    }
                                                    Err(_) => {
                                                        // Все методы не сработали - возможно это протобуф, который мы не распознали
                                                        warn!("MEXC: Все методы декомпрессии не сработали");
                                                        warn!("Возможно это протобуф данные или неподдерживаемый формат");
                                                        warn!("Данные: длина={}, UTF-8 ошибка: {}", binary.len(), utf8_error);
                                                        
                                                        Err(std::io::Error::new(
                                                            std::io::ErrorKind::InvalidData,
                                                            format!("All decompression methods failed: {}", utf8_error),
                                                        ))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    error!("MEXC received empty binary data");
                                    Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Empty binary data from MEXC",
                                    ))
                                }
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
                    self.set_connection_state(ConnectionState::Failed("Reconnection failed".to_string()));
                    log_connection_event(self.exchange, "connection_failed_final", "Exiting after failed reconnection attempts");
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
        log_connection_event(self.exchange, "close_requested", "Closing WebSocket connection");
        self.set_connection_state(ConnectionState::Disconnected);
        
        // Graceful shutdown ping задачи
        self.stop_ping_task_safely();

        // close the websocket connection and break the while loop in run()
        _ = self.command_tx.send(Message::Close(None)).await;
        
        log_connection_event(self.exchange, "close_completed", "WebSocket connection closed");
    }
}
