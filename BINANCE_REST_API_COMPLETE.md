# Binance REST API - Полная документация для разработчиков

## Обзор

Данная документация описывает использование всех Binance REST клиентов в крейте `crypto-rest-client`. Binance поддерживает четыре основных типа рынков через отдельные REST API эндпоинты.

## Поддерживаемые клиенты

1. **BinanceSpotRestClient** - Спотовый рынок
2. **BinanceLinearRestClient** - USDT-маржинальные фьючерсы и свопы
3. **BinanceInverseRestClient** - Coin-маржинальные фьючерсы и свопы
4. **BinanceOptionRestClient** - Европейские опционы

## 1. BinanceSpotRestClient - Спотовый рынок

### Основная информация

-   **Базовый URL:** `https://api.binance.com`
-   **Документация API:** https://binance-docs.github.io/apidocs/spot/en/
-   **Торговая платформа:** https://www.binance.com/en/trade/BTC_USDT
-   **Лимиты запросов:** 1200 request weight в минуту, 6100 сырых запросов за 5 минут

### Создание клиента

```rust
use crypto_rest_client::BinanceSpotRestClient;

// Создание клиента без аутентификации (только публичные методы)
let client = BinanceSpotRestClient::new(None, None, None);

// Создание клиента с API ключами для приватных методов
let client = BinanceSpotRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
    None, // без прокси
);

// Создание клиента с прокси
let client = BinanceSpotRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
    Some("http://proxy.example.com:8080".to_string()),
);
```

### Публичные методы

#### Получение агрегированных сделок

```rust
/// Получает сжатые агрегированные сделки
///
/// Эндпоинт: GET /api/v3/aggTrades
/// Лимит: 1000 сделок
pub async fn fetch_agg_trades(
    symbol: &str,
    from_id: Option<u64>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<String>
```

**Пример использования:**

```rust
use crypto_rest_client::BinanceSpotRestClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получение последних 1000 сделок
    let trades = BinanceSpotRestClient::fetch_agg_trades(
        "BTCUSDT",
        None,
        None,
        None,
    ).await?;

    println!("Spot Trades: {}", trades);
    Ok(())
}
```

#### Получение снапшота стакана заявок

```rust
/// Получает снапшот стакана заявок Level2
///
/// Эндпоинт: GET /api/v3/depth
/// Лимит: 1000 уровней
pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String>
```

**Пример использования:**

```rust
let orderbook = BinanceSpotRestClient::fetch_l2_snapshot("BTCUSDT").await?;
println!("Spot Orderbook: {}", orderbook);
```

### Приватные методы (требуют аутентификации)

#### Получение баланса аккаунта

```rust
/// Получает баланс конкретного актива
///
/// Эндпоинт: GET /api/v3/account
pub async fn get_account_balance(&self, asset: &str) -> Result<String>
```

**Пример использования:**

```rust
let client = BinanceSpotRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
    None,
);

let btc_balance = client.get_account_balance("BTC").await?;
println!("BTC Balance: {}", btc_balance);
```

#### Создание лимитного ордера

```rust
/// Создает лимитный ордер
///
/// Эндпоинт: POST /api/v3/order
pub async fn create_order(
    &self,
    symbol: &str,
    side: &str,
    quantity: f64,
    price: f64,
    _market_type: &str,
) -> Result<String>
```

**Пример использования:**

```rust
let buy_order = client.create_order(
    "BTCUSDT",
    "BUY",
    0.001,      // количество BTC
    50000.0,    // цена в USDT
    "spot",     // тип рынка
).await?;

println!("Spot Buy Order: {}", buy_order);
```

#### Создание рыночного ордера

```rust
/// Создает рыночный ордер
///
/// Эндпоинт: POST /api/v3/order
pub async fn create_market_order(
    &self,
    symbol: &str,
    side: &str,
    quantity: f64,
) -> Result<String>
```

**Пример использования:**

```rust
let market_order = client.create_market_order(
    "BTCUSDT",
    "BUY",
    0.001,  // количество BTC
).await?;

println!("Spot Market Order: {}", market_order);
```

## 2. BinanceLinearRestClient - USDT-маржинальные фьючерсы и свопы

### Основная информация

-   **Базовый URL:** `https://fapi.binance.com`
-   **Документация API:** https://binance-docs.github.io/apidocs/futures/en/
-   **Торговая платформа:** https://www.binance.com/en/futures/BTC_USDT
-   **Лимиты запросов:** 2400 request weight в минуту

### Создание клиента

```rust
use crypto_rest_client::BinanceLinearRestClient;

let client = BinanceLinearRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
    None,
);
```

### Публичные методы

#### Получение информации о бирже

```rust
/// Получает текущие правила торговли и информацию о символах
///
/// Эндпоинт: GET /fapi/v1/exchangeInfo
pub async fn get_exchange_info(&self) -> Result<String>
```

#### Получение агрегированных сделок

```rust
/// Получает сжатые агрегированные сделки
///
/// Эндпоинт: GET /fapi/v1/aggTrades
pub async fn fetch_agg_trades(
    symbol: &str,
    from_id: Option<u64>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<String>
```

#### Получение снапшота стакана заявок

```rust
/// Получает снапшот стакана заявок Level2
///
/// Эндпоинт: GET /fapi/v1/depth
pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String>
```

#### Получение открытого интереса

```rust
/// Получает открытый интерес для символа
///
/// Эндпоинт: GET /fapi/v1/openInterest
pub async fn fetch_open_interest(symbol: &str) -> Result<String>
```

### Приватные методы

#### Получение баланса аккаунта

```rust
/// Получает баланс конкретного актива
///
/// Эндпоинт: GET /fapi/v3/balance
pub async fn get_account_balance(&self, asset: &str) -> Result<String>
```

#### Создание лимитного ордера

```rust
/// Создает лимитный ордер
///
/// Эндпоинт: POST /fapi/v1/order
pub async fn create_order(
    &self,
    symbol: &str,
    side: &str,
    quantity: f64,
    price: f64,
    _market_type: &str,
) -> Result<String>
```

#### Создание рыночного ордера

```rust
/// Создает рыночный ордер
///
/// Эндпоинт: POST /fapi/v1/order
pub async fn create_market_order(
    &self,
    symbol: &str,
    side: &str,
    quantity: f64,
) -> Result<String>
```

## 3. BinanceInverseRestClient - Coin-маржинальные фьючерсы и свопы

### Основная информация

-   **Базовый URL:** `https://dapi.binance.com`
-   **Документация API:** https://binance-docs.github.io/apidocs/delivery/en/
-   **Торговая платформа:** https://www.binance.com/en/delivery/btcusd_perpetual
-   **Лимиты запросов:** 2400 request weight в минуту

### Создание клиента

```rust
use crypto_rest_client::BinanceInverseRestClient;

let client = BinanceInverseRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
);
```

### Публичные методы

#### Получение агрегированных сделок

```rust
/// Получает сжатые агрегированные сделки
///
/// Эндпоинт: GET /dapi/v1/aggTrades
/// Лимит: 1000 сделок
pub async fn fetch_agg_trades(
    symbol: &str,
    from_id: Option<u64>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<String>
```

**Пример использования:**

```rust
use crypto_rest_client::BinanceInverseRestClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получение сделок для перпетуального свопа
    let trades = BinanceInverseRestClient::fetch_agg_trades(
        "BTCUSD_PERP",
        None,
        None,
        None,
    ).await?;

    // Получение сделок для фьючерса с экспирацией
    let future_trades = BinanceInverseRestClient::fetch_agg_trades(
        "BTCUSD_210625",
        None,
        None,
        None,
    ).await?;

    println!("Inverse Trades: {}", trades);
    Ok(())
}
```

#### Получение снапшота стакана заявок

```rust
/// Получает снапшот стакана заявок Level2
///
/// Эндпоинт: GET /dapi/v1/depth
/// Лимит: 1000 уровней
pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String>
```

**Пример использования:**

```rust
let orderbook = BinanceInverseRestClient::fetch_l2_snapshot("BTCUSD_PERP").await?;
println!("Inverse Orderbook: {}", orderbook);
```

#### Получение открытого интереса

```rust
/// Получает открытый интерес для символа
///
/// Эндпоинт: GET /dapi/v1/openInterest
pub async fn fetch_open_interest(symbol: &str) -> Result<String>
```

**Пример использования:**

```rust
let open_interest = BinanceInverseRestClient::fetch_open_interest("BTCUSD_PERP").await?;
println!("Inverse Open Interest: {}", open_interest);
```

## 4. BinanceOptionRestClient - Европейские опционы

### Основная информация

-   **Базовый URL:** `https://vapi.binance.com`
-   **Документация API:** https://binance-docs.github.io/apidocs/voptions/en/
-   **Торговая платформа:** https://voptions.binance.com/en

### Создание клиента

```rust
use crypto_rest_client::BinanceOptionRestClient;

let client = BinanceOptionRestClient::new(
    Some("your_api_key".to_string()),
    Some("your_api_secret".to_string()),
);
```

### Публичные методы

#### Получение последних сделок

```rust
/// Получает последние сделки
///
/// Эндпоинт: GET /vapi/v1/trades
/// Лимит: 500 сделок
pub async fn fetch_trades(symbol: &str, start_time: Option<u64>) -> Result<String>
```

**Пример использования:**

```rust
use crypto_rest_client::BinanceOptionRestClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получение сделок для опциона
    let trades = BinanceOptionRestClient::fetch_trades(
        "BTC-220624-50000-C",  // Call опцион на BTC
        None,
    ).await?;

    // Получение сделок с определенного времени
    let start_time = 1640995200000; // timestamp в миллисекундах
    let trades_from_time = BinanceOptionRestClient::fetch_trades(
        "BTC-220624-50000-P",  // Put опцион на BTC
        Some(start_time),
    ).await?;

    println!("Option Trades: {}", trades);
    Ok(())
}
```

#### Получение снапшота стакана заявок

```rust
/// Получает снапшот стакана заявок Level2
///
/// Эндпоинт: GET /vapi/v1/depth
/// Лимит: 1000 уровней
pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String>
```

**Пример использования:**

```rust
let orderbook = BinanceOptionRestClient::fetch_l2_snapshot("BTC-220624-50000-C").await?;
println!("Option Orderbook: {}", orderbook);
```

## Сравнение клиентов

| Клиент  | Базовый URL      | Типы ордеров       | Открытый интерес | Комиссии       |
| ------- | ---------------- | ------------------ | ---------------- | -------------- |
| Spot    | api.binance.com  | Лимитные, Рыночные | ❌               | 0.1% / 0.1%    |
| Linear  | fapi.binance.com | Лимитные, Рыночные | ✅               | 0.02% / 0.04%  |
| Inverse | dapi.binance.com | Только публичные   | ✅               | 0.015% / 0.04% |
| Option  | vapi.binance.com | Только публичные   | ❌               | Индивидуально  |

## Форматы символов

### Спотовый рынок

-   **Формат:** `BTCUSDT`, `ETHUSDT`, `ADAUSDT`
-   **Примеры:** `BTCUSDT`, `ETHBTC`, `BNBUSDT`

### Linear (USDT-маржинальные)

-   **Перпетуальные свопы:** `BTCUSDT`, `ETHUSDT`
-   **Фьючерсы с экспирацией:** `BTCUSDT_210625`, `ETHUSDT_220930`

### Inverse (Coin-маржинальные)

-   **Перпетуальные свопы:** `BTCUSD_PERP`, `ETHUSD_PERP`
-   **Фьючерсы с экспирацией:** `BTCUSD_210625`, `ETHUSD_220930`

### Опционы

-   **Call опционы:** `BTC-220624-50000-C`
-   **Put опционы:** `BTC-220624-50000-P`
-   **Формат:** `{UNDERLYING}-{EXPIRY}-{STRIKE}-{TYPE}`

## Универсальные функции

Крейт также предоставляет универсальные функции для работы с разными типами рынков:

```rust
use crypto_rest_client::binance::{fetch_l2_snapshot, fetch_open_interest};
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получение стакана для разных типов рынков
    let spot_book = fetch_l2_snapshot(MarketType::Spot, "BTCUSDT").await?;
    let linear_book = fetch_l2_snapshot(MarketType::LinearSwap, "BTCUSDT").await?;
    let inverse_book = fetch_l2_snapshot(MarketType::InverseSwap, "BTCUSD_PERP").await?;
    let option_book = fetch_l2_snapshot(MarketType::EuropeanOption, "BTC-220624-50000-C").await?;

    // Получение открытого интереса (только для фьючерсов и свопов)
    let linear_oi = fetch_open_interest(MarketType::LinearSwap, "BTCUSDT").await?;
    let inverse_oi = fetch_open_interest(MarketType::InverseSwap, "BTCUSD_PERP").await?;

    println!("Spot Orderbook: {}", spot_book);
    println!("Linear Open Interest: {}", linear_oi);

    Ok(())
}
```

## Полный пример использования всех клиентов

```rust
use crypto_rest_client::{
    BinanceSpotRestClient, BinanceLinearRestClient,
    BinanceInverseRestClient, BinanceOptionRestClient
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Анализ всех рынков Binance ===\n");

    // 1. Спотовый рынок
    println!("1. Спотовый рынок:");
    let spot_trades = BinanceSpotRestClient::fetch_agg_trades("BTCUSDT", None, None, None).await?;
    let spot_orderbook = BinanceSpotRestClient::fetch_l2_snapshot("BTCUSDT").await?;
    println!("  Сделки получены: {} символов", spot_trades.len());
    println!("  Стакан получен: {} символов", spot_orderbook.len());

    // 2. Linear фьючерсы
    println!("\n2. USDT-маржинальные фьючерсы:");
    let linear_trades = BinanceLinearRestClient::fetch_agg_trades("BTCUSDT", None, None, None).await?;
    let linear_orderbook = BinanceLinearRestClient::fetch_l2_snapshot("BTCUSDT").await?;
    let linear_oi = BinanceLinearRestClient::fetch_open_interest("BTCUSDT").await?;
    println!("  Сделки получены: {} символов", linear_trades.len());
    println!("  Стакан получен: {} символов", linear_orderbook.len());
    println!("  Открытый интерес получен: {} символов", linear_oi.len());

    // 3. Inverse фьючерсы
    println!("\n3. Coin-маржинальные фьючерсы:");
    let inverse_trades = BinanceInverseRestClient::fetch_agg_trades("BTCUSD_PERP", None, None, None).await?;
    let inverse_orderbook = BinanceInverseRestClient::fetch_l2_snapshot("BTCUSD_PERP").await?;
    let inverse_oi = BinanceInverseRestClient::fetch_open_interest("BTCUSD_PERP").await?;
    println!("  Сделки получены: {} символов", inverse_trades.len());
    println!("  Стакан получен: {} символов", inverse_orderbook.len());
    println!("  Открытый интерес получен: {} символов", inverse_oi.len());

    // 4. Опционы
    println!("\n4. Европейские опционы:");
    let option_trades = BinanceOptionRestClient::fetch_trades("BTC-220624-50000-C", None).await?;
    let option_orderbook = BinanceOptionRestClient::fetch_l2_snapshot("BTC-220624-50000-C").await?;
    println!("  Сделки получены: {} символов", option_trades.len());
    println!("  Стакан получен: {} символов", option_orderbook.len());

    println!("\n=== Анализ завершен ===");

    Ok(())
}
```

## Обработка ошибок

### Типичные ошибки для всех клиентов

```rust
use crypto_rest_client::BinanceSpotRestClient;
use serde_json::Value;

#[tokio::main]
async fn main() {
    match BinanceSpotRestClient::fetch_l2_snapshot("INVALID_SYMBOL").await {
        Ok(response) => {
            // Проверяем на ошибки API в ответе
            if let Ok(json) = serde_json::from_str::<Value>(&response) {
                if let Some(code) = json.get("code") {
                    match code.as_i64().unwrap() {
                        -1121 => println!("Ошибка: Неверный символ"),
                        -1003 => println!("Ошибка: Превышен лимит запросов"),
                        -1022 => println!("Ошибка: Неверная подпись"),
                        _ => println!("Другая ошибка API: {}", json.get("msg").unwrap()),
                    }
                } else {
                    println!("Успешный ответ: {}", response);
                }
            }
        }
        Err(e) => {
            println!("Ошибка сети: {}", e);
        }
    }
}
```

## Аутентификация

### Процесс подписи (для приватных методов)

1. **Создание строки параметров:** `symbol=BTCUSDT&side=BUY&type=LIMIT&quantity=0.001&price=50000&timestamp=1640995200000`
2. **Генерация подписи:** HMAC-SHA256 с использованием API Secret
3. **Добавление подписи:** `&signature=generated_signature`
4. **Заголовки:** `X-MBX-APIKEY: your_api_key`

### Пример с аутентификацией

```rust
use crypto_rest_client::BinanceSpotRestClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = BinanceSpotRestClient::new(
        Some("your_api_key".to_string()),
        Some("your_api_secret".to_string()),
        None,
    );

    // Приватные методы требуют аутентификации
    let balance = client.get_account_balance("BTC").await?;
    println!("BTC Balance: {}", balance);

    // Создание ордера
    let order = client.create_order(
        "BTCUSDT",
        "BUY",
        0.001,
        50000.0,
        "spot"
    ).await?;
    println!("Order created: {}", order);

    Ok(())
}
```

## Лимиты и ограничения

| Клиент  | Лимит запросов                      | Лимит ордеров           | Особенности           |
| ------- | ----------------------------------- | ----------------------- | --------------------- |
| Spot    | 1200 weight/мин, 6100 запросов/5мин | Зависит от VIP уровня   | Самые строгие лимиты  |
| Linear  | 2400 weight/мин                     | 300 ордеров/10сек       | Маржинальная торговля |
| Inverse | 2400 weight/мин                     | Только публичные методы | Coin-маржинальные     |
| Option  | Не указано                          | Только публичные методы | Европейские опционы   |

## Полезные ссылки

-   [Binance Spot REST API](https://binance-docs.github.io/apidocs/spot/en/)
-   [Binance Futures REST API](https://binance-docs.github.io/apidocs/futures/en/)
-   [Binance Delivery REST API](https://binance-docs.github.io/apidocs/delivery/en/)
-   [Binance Options REST API](https://binance-docs.github.io/apidocs/voptions/en/)
-   [Комиссии Binance](https://www.binance.com/en/fee/schedule)

## Примечания

1. **Тестовые сети:** Каждый тип рынка имеет свою тестовую сеть
2. **API ключи:** Для каждого типа рынка нужны отдельные API ключи
3. **Лимиты:** Соблюдайте лимиты запросов для избежания блокировки
4. **Символы:** Используйте правильные форматы символов для каждого типа рынка
5. **Безопасность:** Никогда не публикуйте API ключи в коде
