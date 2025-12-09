# WebSocket Connection Issues - Диагностика и решения

## Обзор проблем

На основе анализа логов и кода выявлены две основные проблемы с WebSocket соединениями:

1. **Connection reset without closing handshake** - неожиданное закрытие соединения
2. **Failed to send shutdown signal to ping task: channel closed** - проблемы с управлением ping задачами

## Анализ ошибок

### 1. Connection Reset Without Closing Handshake

```
[2025-12-09T11:02:31Z ERROR crypto_ws_client::common::connect_async] 
Failed to read, error: WebSocket protocol error: Connection reset without closing handshake
```

**Причины:**
- Сервер принудительно закрыл соединение без WebSocket handshake
- Сетевые проблемы (таймауты, потеря пакетов)
- Превышение лимитов API (rate limiting)
- Проблемы с прокси (если используется)

**Местоположение в коде:**
```rust
// crypto-ws-client/src/common/connect_async.rs:116-119
Some(Err(err)) => {
  error!("Failed to read, error: {}", err);
  break;
}
```

### 2. Ping Task Channel Closed

```
[2025-12-09T11:02:32Z ERROR crypto_ws_client::common::ws_client_internal] 
Failed to send shutdown signal to ping task: channel closed
```

**Причины:**
- Ping задача уже завершилась до отправки сигнала остановки
- Канал был закрыт из-за предыдущих ошибок соединения
- Состояние гонки между переподключением и остановкой ping задачи

**Местоположение в коде:**
```rust
// crypto-ws-client/src/common/ws_client_internal.rs:456-458
if let Err(err) = tx.send(true) {
    error!("Failed to send shutdown signal to ping task: {}", err);
}
```

## Архитектура WebSocket клиента

### Основные компоненты

1. **WSClientInternal** - основной клиент с переподключением
2. **connect_async** - установка соединения с поддержкой прокси
3. **Ping Task** - поддержание соединения alive
4. **Message Handler** - обработка входящих сообщений

### Жизненный цикл соединения

```rust
// Упрощенная схема
WSClientInternal::connect()
    ↓
connect_async() → (message_rx, command_tx)
    ↓
WSClientInternal::run()
    ↓
start_ping_task() + message processing loop
    ↓
При ошибке → reconnect() → повтор цикла
```

## Проблемы в текущей реализации

### 1. Управление Ping задачами

**Проблема:** Состояние гонки при остановке ping задач

```rust
// Текущий код - ПРОБЛЕМАТИЧНЫЙ
if let Err(err) = tx.send(true) {
    error!("Failed to send shutdown signal to ping task: {}", err);
}
```

**Решение:** Улучшенная обработка состояний

```rust
// Предлагаемое исправление
match tx.send(true) {
    Ok(_) => {
        debug!("Successfully sent shutdown signal to ping task");
    }
    Err(tokio::sync::watch::error::SendError(_)) => {
        // Канал уже закрыт - это нормально при быстром переподключении
        debug!("Ping task channel already closed - task likely finished");
    }
}
```

### 2. Обработка неожиданных отключений

**Проблема:** Недостаточно graceful handling при connection reset

```rust
// Текущий код
Some(Err(err)) => {
  error!("Failed to read, error: {}", err);
  break; // Просто выходим из цикла
}
```

**Решение:** Классификация ошибок и соответствующие действия

```rust
// Предлагаемое улучшение
Some(Err(err)) => {
    match err {
        Error::ConnectionClosed | Error::Protocol(_) => {
            warn!("Connection lost: {}, attempting reconnect", err);
            // Инициируем переподключение
            break;
        }
        Error::Io(io_err) if io_err.kind() == std::io::ErrorKind::UnexpectedEof => {
            warn!("Unexpected EOF, server closed connection: {}", err);
            break;
        }
        _ => {
            error!("Unrecoverable WebSocket error: {}", err);
            return; // Полный выход
        }
    }
}
```

### 3. Переподключение и состояние

**Проблема:** Сложная логика переподключения с потенциальными deadlock'ами

```rust
// Текущая проблематичная логика
if !self.reconnect_in_progress.load(Ordering::SeqCst) {
    // Переподключение...
} else {
    // Ждем и выходим...
}
```

## Рекомендуемые исправления

### 1. Улучшенная обработка ping задач

```rust
impl<H: MessageHandler> WSClientInternal<H> {
    fn stop_ping_task_safely(&self) {
        let mut guard = self.ping_task_handle.lock().unwrap();
        if let Some(handle) = guard.take() {
            // Сначала пытаемся graceful shutdown
            if let Some(shutdown_tx) = self.ping_shutdown_tx.lock().unwrap().take() {
                match shutdown_tx.send(true) {
                    Ok(_) => {
                        debug!("Sent graceful shutdown signal to ping task");
                        // Даем время на graceful shutdown
                        tokio::time::timeout(
                            Duration::from_secs(2), 
                            async { handle.await }
                        ).await.unwrap_or_else(|_| {
                            warn!("Ping task didn't shutdown gracefully, aborting");
                            handle.abort();
                        });
                    }
                    Err(_) => {
                        debug!("Ping task channel closed, aborting task");
                        handle.abort();
                    }
                }
            } else {
                // Нет канала - просто abort
                handle.abort();
            }
        }
    }
}
```

### 2. Улучшенная обработка соединений

```rust
async fn connect_async_with_retry(
    url: &str,
    uplink_limit: Option<(NonZeroU32, std::time::Duration)>,
    max_retries: u32,
) -> Result<(Receiver<Message>, Sender<Message>), Error> {
    let mut backoff = Duration::from_secs(1);
    
    for attempt in 1..=max_retries {
        match connect_async(url, uplink_limit).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                match err {
                    Error::Http(resp) if resp.status() == StatusCode::TOO_MANY_REQUESTS => {
                        let retry_after = extract_retry_after(&resp).unwrap_or(60);
                        warn!("Rate limited, waiting {} seconds", retry_after);
                        tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    }
                    Error::Io(_) | Error::Protocol(_) => {
                        if attempt < max_retries {
                            warn!("Connection failed (attempt {}/{}): {}", attempt, max_retries, err);
                            tokio::time::sleep(backoff).await;
                            backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                        }
                    }
                    _ => return Err(err), // Неисправимые ошибки
                }
            }
        }
    }
    
    Err(Error::ConnectionClosed)
}
```

### 3. Состояние соединения

```rust
#[derive(Debug, Clone, PartialEq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

impl<H: MessageHandler> WSClientInternal<H> {
    fn set_connection_state(&self, state: ConnectionState) {
        let mut guard = self.connection_state.lock().unwrap();
        if *guard != state {
            info!("Connection state changed: {:?} -> {:?}", *guard, state);
            *guard = state;
        }
    }
}
```

## Мониторинг и диагностика

### 1. Метрики соединения

```rust
#[derive(Debug, Default)]
struct ConnectionMetrics {
    total_connections: AtomicU64,
    successful_connections: AtomicU64,
    failed_connections: AtomicU64,
    reconnection_attempts: AtomicU64,
    ping_failures: AtomicU64,
    last_error: Mutex<Option<String>>,
}

impl ConnectionMetrics {
    fn record_connection_attempt(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }
    
    fn record_connection_success(&self) {
        self.successful_connections.fetch_add(1, Ordering::Relaxed);
    }
    
    fn record_connection_failure(&self, error: &str) {
        self.failed_connections.fetch_add(1, Ordering::Relaxed);
        *self.last_error.lock().unwrap() = Some(error.to_string());
    }
}
```

### 2. Логирование

```rust
// Структурированное логирование
fn log_connection_event(exchange: &str, event: &str, details: &str) {
    info!(
        target: "websocket_connection",
        exchange = exchange,
        event = event,
        details = details,
        timestamp = chrono::Utc::now().to_rfc3339(),
        "WebSocket connection event"
    );
}
```

### 3. Health Check

```rust
impl<H: MessageHandler> WSClientInternal<H> {
    pub fn get_health_status(&self) -> HealthStatus {
        let state = self.connection_state.lock().unwrap().clone();
        let metrics = &self.metrics;
        
        HealthStatus {
            state,
            total_connections: metrics.total_connections.load(Ordering::Relaxed),
            successful_connections: metrics.successful_connections.load(Ordering::Relaxed),
            failed_connections: metrics.failed_connections.load(Ordering::Relaxed),
            last_ping: self.last_ping_time.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed(),
        }
    }
}
```

## Рекомендации по использованию

### 1. Обработка ошибок в приложении

```rust
use crypto_ws_client::WSClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let ws_client = BinanceSpotWSClient::new(tx, None).await;
    
    // Запускаем клиент в отдельной задаче
    let client_handle = tokio::spawn(async move {
        ws_client.run().await;
    });
    
    // Обрабатываем сообщения с таймаутом
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(message) => {
                        println!("Received: {}", message);
                    }
                    None => {
                        warn!("Message channel closed, client stopped");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                debug!("No messages received in 30 seconds - this is normal");
            }
        }
    }
    
    // Ждем завершения клиента
    if let Err(e) = client_handle.await {
        error!("WebSocket client task failed: {}", e);
    }
    
    Ok(())
}
```

### 2. Конфигурация для продакшена

```rust
// Рекомендуемые настройки для стабильной работы
const PRODUCTION_CONFIG: WSConfig = WSConfig {
    max_reconnection_attempts: 10,
    initial_backoff: Duration::from_secs(2),
    max_backoff: Duration::from_secs(60),
    ping_interval: Duration::from_secs(30),
    ping_timeout: Duration::from_secs(10),
    message_buffer_size: 1000,
    enable_compression: true,
    rate_limit_buffer: Duration::from_secs(5),
};
```

### 3. Мониторинг в продакшене

```rust
// Периодическая проверка состояния
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        let health = ws_client.get_health_status();
        
        if health.failed_connections > health.successful_connections * 2 {
            warn!("High failure rate detected: {}/{}", 
                  health.failed_connections, health.total_connections);
        }
        
        if health.last_ping + 120 < chrono::Utc::now().timestamp() {
            warn!("No ping response for over 2 minutes");
        }
    }
});
```

## Заключение

Основные проблемы связаны с:

1. **Недостаточно graceful обработкой отключений** - нужно различать типы ошибок
2. **Состоянием гонки в ping задачах** - требуется улучшенная синхронизация
3. **Отсутствием детального мониторинга** - нужны метрики и health checks

Рекомендуется:
- Реализовать предложенные исправления поэтапно
- Добавить comprehensive логирование и метрики
- Протестировать в условиях нестабильной сети
- Настроить мониторинг для продакшена
