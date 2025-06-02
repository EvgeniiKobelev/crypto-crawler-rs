# Crypto Client

Унифицированный Rust клиент для работы с различными криптовалютными биржами через REST API и WebSocket соединения.

## Особенности

-   🔗 **Единый интерфейс** для работы с множеством бирж
-   🔄 **REST API и WebSocket** поддержка
-   ⚡ **Асинхронный** клиент с использованием Tokio
-   🛡️ **Безопасность** с поддержкой API ключей и прокси
-   📦 **Модульная архитектура** для легкого расширения
-   🎯 **Типобезопасность** с использованием строгой типизации Rust

## Поддерживаемые биржи

### REST API

-   Binance (Spot, Linear, Inverse, Option)
-   OKX
-   Bybit
-   Huobi
-   KuCoin
-   MEXC (Spot, Swap)
-   Bingx (Spot, Swap)
-   Bitfinex
-   Bitget (Spot, Swap)
-   Bithumb
-   BitMEX
-   Bitstamp
-   Bitz (Spot, Swap)
-   Coinbase Pro
-   Deribit
-   FTX
-   Kraken (Spot, Futures)
-   ZB (Spot, Swap)

### WebSocket (в разработке)

-   Binance
-   OKX
-   Bybit
-   Huobi
-   KuCoin
-   MEXC
-   Bitget
-   Kraken
-   Gate

## Установка

Добавьте в ваш `Cargo.toml`:

```toml
[dependencies]
crypto-client = { path = "../crypto-client" }
tokio = { version = "1.0", features = ["full"] }
```

## Быстрый старт

### Простое использование

```rust
use crypto_client::{CryptoClient, ExchangeConfig, ExchangeType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Создание клиента
    let mut client = CryptoClient::new();

    // Настройка биржи
    let config = ExchangeConfig::new(
        Some("your_api_key".to_string()),
        Some("your_secret_key".to_string()),
    );

    // Добавление биржи
    client.rest_client.add_exchange(ExchangeType::BinanceSpot, config)?;

    // Получение снимка orderbook
    let snapshot = client
        .rest_client
        .fetch_l2_snapshot(&ExchangeType::BinanceSpot, "BTCUSDT")
        .await?;

    println!("Orderbook: {}", snapshot);

    Ok(())
}
```

### Конфигурация для нескольких бирж

```rust
use crypto_client::{CryptoClient, ExchangeConfig, ExchangeType, MultiExchangeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MultiExchangeConfig::new()
        .add_exchange(
            ExchangeType::BinanceSpot,
            ExchangeConfig::new(
                Some("binance_api_key".to_string()),
                Some("binance_secret".to_string()),
            ),
        )
        .add_exchange(
            ExchangeType::KucoinSpot,
            ExchangeConfig::with_passphrase(
                Some("kucoin_api_key".to_string()),
                Some("kucoin_secret".to_string()),
                Some("kucoin_passphrase".to_string()),
            ),
        )
        .with_timeout(60)
        .with_retry_attempts(3);

    let client = CryptoClient::from_config(config)?;

    // Получение данных со всех настроенных бирж
    let results = client
        .rest_client
        .fetch_all_l2_snapshots("BTCUSDT")
        .await;

    for (exchange, result) in results {
        match result {
            Ok(data) => println!("{:?}: {}", exchange, data),
            Err(e) => println!("{:?}: Ошибка - {}", exchange, e),
        }
    }

    Ok(())
}
```

### WebSocket использование (в разработке)

```rust
use crypto_client::{CryptoWsClient, ExchangeConfig, ExchangeType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ws_client = CryptoWsClient::new();

    let config = ExchangeConfig::new(
        Some("api_key".to_string()),
        Some("secret_key".to_string()),
    );

    // Добавление WebSocket клиента
    ws_client.add_exchange(ExchangeType::BinanceSpot, config)?;

    // Подключение
    ws_client.connect_exchange(&ExchangeType::BinanceSpot).await?;

    // Подписка на orderbook
    ws_client.subscribe_orderbook(&ExchangeType::BinanceSpot, "BTCUSDT").await?;

    // Получение сообщений
    while let Ok(Some(message)) = ws_client.next_message().await {
        println!("Получено: {:?}", message);
    }

    Ok(())
}
```

## Архитектура

Проект организован в модули:

### Основные модули

-   **`exchange_type`** - Перечисление всех поддерживаемых бирж
-   **`config`** - Конфигурация для подключения к биржам
-   **`traits`** - Общие трейты для клиентов
-   **`rest_client`** - REST API клиенты и фабрика
-   **`ws_client`** - WebSocket клиенты (в разработке)

### Основные структуры

-   **`CryptoClient`** - Главный клиент, содержащий REST и WS клиенты
-   **`CryptoRestClient`** - Менеджер REST API клиентов
-   **`CryptoWsClient`** - Менеджер WebSocket клиентов
-   **`ExchangeConfig`** - Конфигурация подключения к бирже
-   **`MultiExchangeConfig`** - Конфигурация для нескольких бирж

## Обработка ошибок

Библиотека использует кастомный тип ошибок `ExchangeError`:

```rust
use crypto_client::{ExchangeError, ExchangeResult};

fn handle_result(result: ExchangeResult<String>) {
    match result {
        Ok(data) => println!("Успех: {}", data),
        Err(ExchangeError::NetworkError(msg)) => println!("Сетевая ошибка: {}", msg),
        Err(ExchangeError::AuthError(msg)) => println!("Ошибка аутентификации: {}", msg),
        Err(ExchangeError::ApiError(msg)) => println!("Ошибка API: {}", msg),
        Err(e) => println!("Другая ошибка: {}", e),
    }
}
```

## Примеры

Смотрите папку `examples/` для подробных примеров использования:

-   `basic_usage.rs` - Базовое использование REST API
-   Планируется: `websocket_usage.rs` - Использование WebSocket
-   Планируется: `multi_exchange.rs` - Работа с несколькими биржами

## Планы развития

-   [ ] Полная реализация WebSocket клиентов
-   [ ] Поддержка торговых операций (размещение ордеров)
-   [ ] Расширенное управление подписками
-   [ ] Автоматическое переподключение WebSocket
-   [ ] Метрики и мониторинг
-   [ ] Документация API для каждой биржи

## Требования

-   Rust 1.70+
-   Tokio runtime для асинхронных операций

## Лицензия

Этот проект распространяется под лицензией, указанной в основном проекте.

## Вклад в развитие

Мы приветствуем вклад в развитие проекта! Пожалуйста:

1. Форкните репозиторий
2. Создайте ветку для новой функции
3. Добавьте тесты для новой функциональности
4. Убедитесь, что все тесты проходят
5. Отправьте Pull Request

## Поддержка

Если у вас есть вопросы или проблемы, пожалуйста, создайте issue в репозитории проекта.
