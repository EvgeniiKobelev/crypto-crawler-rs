# Binance WebSocket API - Документация для разработчиков

## Обзор

Данная документация описывает использование Binance WebSocket клиентов в крейте `crypto-ws-client`. Binance поддерживает три типа рынков через отдельные WebSocket эндпоинты.

## Типы клиентов

### 1. BinanceSpotWSClient - Спотовый рынок
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};

// Создание клиента
let (tx, rx) = std::sync::mpsc::channel();
let ws_client = BinanceSpotWSClient::new(tx, None).await;
```

**URL:** `wss://stream.binance.com:9443/stream`  
**Документация:** https://binance-docs.github.io/apidocs/spot/en/  
**Торговля:** https://www.binance.com/en/trade/BTC_USDT

### 2. BinanceLinearWSClient - USDT-маржинальные фьючерсы и свопы
```rust
use crypto_ws_client::{BinanceLinearWSClient, WSClient};

let (tx, rx) = std::sync::mpsc::channel();
let ws_client = BinanceLinearWSClient::new(tx, None).await;
```

**URL:** `wss://fstream.binance.com/stream`  
**Документация:** https://binance-docs.github.io/apidocs/futures/en/  
**Торговля:** https://www.binance.com/en/futures/BTC_USDT

### 3. BinanceInverseWSClient - Coin-маржинальные фьючерсы и свопы
```rust
use crypto_ws_client::{BinanceInverseWSClient, WSClient};

let (tx, rx) = std::sync::mpsc::channel();
let ws_client = BinanceInverseWSClient::new(tx, None).await;
```

**URL:** `wss://dstream.binance.com/stream`  
**Документация:** https://binance-docs.github.io/apidocs/delivery/en/  
**Торговля:** https://www.binance.com/en/delivery/btcusd_quarter

## Поддерживаемые каналы

### Публичные каналы

#### 1. Сделки (Trades)
```rust
// Подписка на агрегированные сделки
ws_client.subscribe_trade(&["BTCUSDT".to_string(), "ETHUSDT".to_string()]).await;

// Внутренний канал: "aggTrade"
// Формат: symbol@aggTrade (например: btcusdt@aggTrade)
```

#### 2. Стакан заявок (Order Book)
```rust
// Инкрементальные обновления стакана (100ms)
ws_client.subscribe_orderbook(&["BTCUSDT".to_string()]).await;

// Топ-K снапшот стакана (20 уровней)
ws_client.subscribe_orderbook_topk(&["BTCUSDT".to_string()]).await;

// Внутренние каналы: "depth@100ms", "depth20"
```

#### 3. Тикеры (24hr statistics)
```rust
// 24-часовая статистика
ws_client.subscribe_ticker(&["BTCUSDT".to_string()]).await;

// Все тикеры сразу
ws_client.send(&[r#"{"id":9527,"method":"SUBSCRIBE","params":["!ticker@arr"]}"#.to_string()]).await;

// Внутренний канал: "ticker"
```

#### 4. Лучшие цены покупки/продажи (BBO)
```rust
// Best Bid/Offer для конкретных символов
ws_client.subscribe_bbo(&["BTCUSDT".to_string()]).await;

// Все BBO сразу (устарело с 7 декабря 2022)
// ws_client.send(&[r#"{"id":9527,"method":"SUBSCRIBE","params":["!bookTicker"]}"#.to_string()]).await;

// Внутренний канал: "bookTicker"
```

#### 5. Свечи (Candlesticks/Klines)
```rust
// Подписка на свечи с интервалами в секундах
ws_client.subscribe_candlestick(&[
    ("BTCUSDT".to_string(), 60),    // 1 минута
    ("ETHUSDT".to_string(), 3600),  // 1 час
    ("ADAUSDT".to_string(), 86400), // 1 день
]).await;

// Поддерживаемые интервалы:
// 60 -> "1m", 180 -> "3m", 300 -> "5m", 900 -> "15m", 1800 -> "30m"
// 3600 -> "1h", 7200 -> "2h", 14400 -> "4h", 21600 -> "6h", 28800 -> "8h", 43200 -> "12h"
// 86400 -> "1d", 259200 -> "3d", 604800 -> "1w", 2592000 -> "1M"
```

### Приватные каналы

#### User Data Stream
```rust
// Получение listen_key через REST API (не входит в данный крейт)
let listen_key = "your_listen_key_from_rest_api";

// Подписка на приватные данные пользователя
ws_client.subscribe_user_data(&listen_key).await;

// Этот канал включает:
// - Обновления баланса аккаунта
// - Обновления ордеров
// - Исполненные сделки
```

## Ограничения и лимиты

### Лимиты подключения
- **Входящие сообщения:** 5 сообщений в секунду
- **Размер WebSocket фрейма:** максимум 4096 байт
- **Максимум топиков в одной подписке:**
  - Спот: 1024 топика
  - Фьючерсы/Свопы: 200 топиков

### Формат команд
```json
{
  "id": 9527,
  "method": "SUBSCRIBE",
  "params": ["btcusdt@aggTrade", "btcusdt@ticker"]
}
```

## Примеры использования

### Базовое подключение и подписка
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};
use std::sync::mpsc;

#[tokio::main]
async fn main() {
    // Создаем канал для получения сообщений
    let (tx, rx) = mpsc::channel();
    
    // Создаем WebSocket клиент
    let ws_client = BinanceSpotWSClient::new(tx, None).await;
    
    // Подписываемся на сделки и тикеры
    ws_client.subscribe_trade(&["BTCUSDT".to_string()]).await;
    ws_client.subscribe_ticker(&["BTCUSDT".to_string()]).await;
    
    // Запускаем клиент в фоновом режиме
    tokio::spawn(async move {
        ws_client.run().await;
    });
    
    // Обрабатываем входящие сообщения
    while let Ok(message) = rx.recv() {
        println!("Получено сообщение: {}", message);
    }
}
```

### Использование с прокси
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};

#[tokio::main]
async fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    
    // Создание клиента с прокси
    let ws_client = BinanceSpotWSClient::new_with_proxy(
        tx,
        None,
        "socks5://username:password@proxy.example.com:1080"
    ).await;
    
    ws_client.subscribe_trade(&["BTCUSDT".to_string()]).await;
    
    tokio::spawn(async move {
        ws_client.run().await;
    });
    
    while let Ok(message) = rx.recv() {
        println!("Сообщение через прокси: {}", message);
    }
}
```

### Приватные данные пользователя
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};

#[tokio::main]
async fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    let ws_client = BinanceSpotWSClient::new(tx, None).await;
    
    // Получите listen_key через REST API
    // Например, GET /api/v3/userDataStream
    let listen_key = "your_listen_key_here";
    
    // Подписка на приватные данные
    ws_client.subscribe_user_data(&listen_key).await;
    
    tokio::spawn(async move {
        ws_client.run().await;
    });
    
    while let Ok(message) = rx.recv() {
        println!("Приватные данные: {}", message);
    }
}
```

### Множественные подписки
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};

#[tokio::main]
async fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    let ws_client = BinanceSpotWSClient::new(tx, None).await;
    
    // Подписка на несколько типов данных
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string(), "ADAUSDT".to_string()];
    
    ws_client.subscribe_trade(&symbols).await;
    ws_client.subscribe_orderbook(&symbols).await;
    ws_client.subscribe_ticker(&symbols).await;
    
    // Подписка на свечи разных интервалов
    ws_client.subscribe_candlestick(&[
        ("BTCUSDT".to_string(), 60),     // 1m
        ("BTCUSDT".to_string(), 300),    // 5m
        ("ETHUSDT".to_string(), 3600),   // 1h
    ]).await;
    
    tokio::spawn(async move {
        ws_client.run().await;
    });
    
    while let Ok(message) = rx.recv() {
        println!("Сообщение: {}", message);
    }
}
```

### Отправка сырых команд
```rust
use crypto_ws_client::{BinanceSpotWSClient, WSClient};

#[tokio::main]
async fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    let ws_client = BinanceSpotWSClient::new(tx, None).await;
    
    // Отправка сырой JSON команды
    let raw_commands = vec![
        r#"{"id":9527,"method":"SUBSCRIBE","params":["btcusdt@aggTrade","ethusdt@ticker"]}"#.to_string(),
        r#"{"id":9528,"method":"SUBSCRIBE","params":["!ticker@arr"]}"#.to_string(),
    ];
    
    ws_client.send(&raw_commands).await;
    
    tokio::spawn(async move {
        ws_client.run().await;
    });
    
    while let Ok(message) = rx.recv() {
        println!("Сообщение: {}", message);
    }
}
```

## Различия между типами рынков

### Символы
- **Спот:** `BTCUSDT`, `ETHUSDT`
- **Linear (USDT-фьючерсы):** `BTCUSDT`, `ETHUSDT`, `BTCUSDT_221230` (с датой экспирации)
- **Inverse (Coin-фьючерсы):** `btcusd_perp`, `ethusd_perp`, `btcusd_221230`

### Дополнительные каналы для фьючерсов
```rust
// Только для Linear и Inverse клиентов
ws_client.subscribe(&[
    ("markPrice".to_string(), "BTCUSDT".to_string()),     // Mark price
    ("forceOrder".to_string(), "BTCUSDT".to_string()),    // Liquidation orders
]).await;

// Все mark prices сразу
ws_client.send(&[r#"{"id":9527,"method":"SUBSCRIBE","params":["!markPrice@arr"]}"#.to_string()]).await;
```

## Обработка ошибок

### Типичные ошибки
```json
// Превышен лимит размера сообщения
{"code": 3001, "msg": "illegal request"}

// Неверный формат команды
{"code": -1100, "msg": "Illegal characters found in parameter"}

// Неизвестный символ
{"code": -1121, "msg": "Invalid symbol"}
```

### Рекомендации
1. **Проверяйте размер WebSocket фреймов** - не более 4096 байт
2. **Соблюдайте лимит скорости** - не более 5 сообщений в секунду
3. **Используйте правильные символы** для каждого типа рынка
4. **Обновляйте listen_key** для User Data Stream каждые 60 минут

## Полезные ссылки

- [Binance Spot WebSocket API](https://binance-docs.github.io/apidocs/spot/en/)
- [Binance Futures WebSocket API](https://binance-docs.github.io/apidocs/futures/en/)
- [Binance Delivery WebSocket API](https://binance-docs.github.io/apidocs/delivery/en/)
- [Binance Options WebSocket API](https://binance-docs.github.io/apidocs/voptions/en/)

## Примечания

1. **User Data Stream требует REST API** для получения listen_key
2. **Автоматическое переподключение** встроено в клиент
3. **Ping/Pong** обрабатывается автоматически
4. **Все символы должны быть в верхнем регистре** для спота и linear фьючерсов
5. **Inverse символы используют нижний регистр** с подчеркиваниями
