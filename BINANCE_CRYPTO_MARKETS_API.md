# Binance Crypto Markets API - Документация для разработчиков

## Обзор

Данная документация описывает использование Binance модуля в крейте `crypto-markets`. Этот модуль предоставляет функции для получения информации о торговых рынках и символах Binance через REST API.

## Поддерживаемые типы рынков

Binance поддерживает следующие типы рынков:

1. **Spot** - Спотовый рынок
2. **LinearSwap** - USDT-маржинальные перпетуальные свопы  
3. **LinearFuture** - USDT-маржинальные фьючерсы
4. **InverseSwap** - Coin-маржинальные перпетуальные свопы
5. **InverseFuture** - Coin-маржинальные фьючерсы
6. **EuropeanOption** - Европейские опционы

## Основные функции

### 1. Получение символов

```rust
use crypto_markets::fetch_symbols;
use crypto_market_type::MarketType;

// Получение всех спотовых символов
let spot_symbols = fetch_symbols("binance", MarketType::Spot)?;
println!("Спотовые символы: {:?}", spot_symbols);

// Получение символов USDT-фьючерсов
let linear_future_symbols = fetch_symbols("binance", MarketType::LinearFuture)?;
println!("Linear Future символы: {:?}", linear_future_symbols);

// Получение символов перпетуальных свопов
let linear_swap_symbols = fetch_symbols("binance", MarketType::LinearSwap)?;
println!("Linear Swap символы: {:?}", linear_swap_symbols);
```

### 2. Получение полной информации о рынках

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

// Получение всех спотовых рынков
let spot_markets = fetch_markets("binance", MarketType::Spot)?;
println!("Количество спотовых рынков: {}", spot_markets.len());

// Получение информации о конкретном рынке
for market in &spot_markets {
    if market.symbol == "BTCUSDT" {
        println!("BTC/USDT рынок: {:#?}", market);
        break;
    }
}
```

## Структура Market

Каждый рынок представлен структурой `Market` со следующими полями:

```rust
pub struct Market {
    /// Название биржи ("binance")
    pub exchange: String,
    /// Тип рынка (Spot, LinearSwap, etc.)
    pub market_type: MarketType,
    /// Торговый символ (например, "BTCUSDT")
    pub symbol: String,
    /// Базовая валюта ID (например, "BTC")
    pub base_id: String,
    /// Котируемая валюта ID (например, "USDT")
    pub quote_id: String,
    /// Валюта расчетов (для фьючерсов и свопов)
    pub settle_id: Option<String>,
    /// Унифицированное название базовой валюты
    pub base: String,
    /// Унифицированное название котируемой валюты
    pub quote: String,
    /// Унифицированное название валюты расчетов
    pub settle: Option<String>,
    /// Активен ли рынок для торговли
    pub active: bool,
    /// Поддерживается ли маржинальная торговля
    pub margin: bool,
    /// Комиссии
    pub fees: Fees,
    /// Точность цены и количества
    pub precision: Precision,
    /// Лимиты количества
    pub quantity_limit: Option<QuantityLimit>,
    /// Стоимость одного контракта (для деривативов)
    pub contract_value: Option<f64>,
    /// Дата поставки (для фьючерсов и опционов)
    pub delivery_date: Option<u64>,
    /// Оригинальные данные от биржи
    pub info: Map<String, Value>,
}
```

## Детальные примеры по типам рынков

### 1. Спотовый рынок (Spot)

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получаем все спотовые рынки
    let spot_markets = fetch_markets("binance", MarketType::Spot)?;
    
    // Фильтруем только активные рынки
    let active_markets: Vec<_> = spot_markets
        .into_iter()
        .filter(|m| m.active)
        .collect();
    
    println!("Активных спотовых рынков: {}", active_markets.len());
    
    // Анализируем популярные пары
    for market in &active_markets {
        if market.quote == "USDT" && 
           ["BTC", "ETH", "BNB", "ADA", "DOT"].contains(&market.base.as_str()) {
            println!("Рынок: {} | Комиссия maker: {}% | taker: {}%",
                market.symbol,
                market.fees.maker * 100.0,
                market.fees.taker * 100.0
            );
            println!("  Точность цены: {} | Точность количества: {}",
                market.precision.tick_size,
                market.precision.lot_size
            );
            
            if let Some(limits) = &market.quantity_limit {
                println!("  Мин. количество: {:?} | Макс. количество: {:?}",
                    limits.min, limits.max);
            }
        }
    }
    
    Ok(())
}
```

### 2. USDT-маржинальные фьючерсы и свопы (Linear)

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получаем перпетуальные свопы
    let swap_markets = fetch_markets("binance", MarketType::LinearSwap)?;
    println!("Перпетуальных свопов: {}", swap_markets.len());
    
    // Получаем фьючерсы с датой экспирации
    let future_markets = fetch_markets("binance", MarketType::LinearFuture)?;
    println!("Фьючерсов с экспирацией: {}", future_markets.len());
    
    // Анализируем свопы
    for market in &swap_markets {
        if market.base == "BTC" {
            println!("Своп: {} | Валюта расчетов: {:?}",
                market.symbol, market.settle);
            println!("  Комиссия maker: {}% | taker: {}%",
                market.fees.maker * 100.0,
                market.fees.taker * 100.0
            );
            println!("  Стоимость контракта: {:?}", market.contract_value);
        }
    }
    
    // Анализируем фьючерсы
    for market in &future_markets {
        if market.base == "BTC" {
            if let Some(delivery_date) = market.delivery_date {
                let date = chrono::DateTime::from_timestamp_millis(delivery_date as i64)
                    .unwrap()
                    .format("%Y-%m-%d");
                println!("Фьючерс: {} | Дата поставки: {}",
                    market.symbol, date);
            }
        }
    }
    
    Ok(())
}
```

### 3. Coin-маржинальные фьючерсы и свопы (Inverse)

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получаем inverse свопы
    let inverse_swaps = fetch_markets("binance", MarketType::InverseSwap)?;
    println!("Inverse свопов: {}", inverse_swaps.len());
    
    // Получаем inverse фьючерсы
    let inverse_futures = fetch_markets("binance", MarketType::InverseFuture)?;
    println!("Inverse фьючерсов: {}", inverse_futures.len());
    
    for market in &inverse_swaps {
        println!("Inverse своп: {} | Базовая: {} | Котируемая: {} | Расчеты: {:?}",
            market.symbol, market.base, market.quote, market.settle);
        println!("  Стоимость контракта: {:?}", market.contract_value);
        println!("  Комиссия maker: {}% | taker: {}%",
            market.fees.maker * 100.0,
            market.fees.taker * 100.0
        );
    }
    
    Ok(())
}
```

### 4. Европейские опционы (EuropeanOption)

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let option_markets = fetch_markets("binance", MarketType::EuropeanOption)?;
    println!("Опционов: {}", option_markets.len());
    
    for market in &option_markets {
        if market.base == "BTC" {
            if let Some(expiry) = market.delivery_date {
                let date = chrono::DateTime::from_timestamp_millis(expiry as i64)
                    .unwrap()
                    .format("%Y-%m-%d");
                
                println!("Опцион: {} | Экспирация: {}", market.symbol, date);
                println!("  Комиссия maker: {}% | taker: {}%",
                    market.fees.maker * 100.0,
                    market.fees.taker * 100.0
                );
            }
        }
    }
    
    Ok(())
}
```

## Комиссии по типам рынков

| Тип рынка | Maker комиссия | Taker комиссия | Примечание |
|-----------|---------------|---------------|------------|
| Spot | 0.1% | 0.1% | Базовая ставка |
| LinearSwap | 0.02% | 0.04% | USDT-маржинальные |
| LinearFuture | 0.02% | 0.04% | USDT-маржинальные |
| InverseSwap | 0.015% | 0.04% | Coin-маржинальные |
| InverseFuture | 0.015% | 0.04% | Coin-маржинальные |
| EuropeanOption | Индивидуально | Индивидуально | Зависит от контракта |

## Эндпоинты API

### Спотовый рынок
- **URL:** `https://api.binance.com/api/v3/exchangeInfo`
- **Документация:** https://binance-docs.github.io/apidocs/spot/en/#exchange-information

### USDT-маржинальные фьючерсы
- **URL:** `https://fapi.binance.com/fapi/v1/exchangeInfo`
- **Документация:** https://binance-docs.github.io/apidocs/futures/en/#exchange-information

### Coin-маржинальные фьючерсы
- **URL:** `https://dapi.binance.com/dapi/v1/exchangeInfo`
- **Документация:** https://binance-docs.github.io/apidocs/delivery/en/#exchange-information

### Опционы
- **URL:** `https://vapi.binance.com/vapi/v1/exchangeInfo`
- **Документация:** https://binance-docs.github.io/apidocs/voptions/en/#exchange-information

## Фильтрация и обработка данных

### Получение активных рынков с фильтрацией

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

fn filter_active_usdt_pairs(markets: Vec<Market>) -> Vec<Market> {
    markets
        .into_iter()
        .filter(|m| {
            m.active && 
            m.quote == "USDT" &&
            !m.symbol.contains("DOWN") &&  // Исключаем leveraged tokens
            !m.symbol.contains("UP") &&
            !m.symbol.contains("BULL") &&
            !m.symbol.contains("BEAR")
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let all_spot_markets = fetch_markets("binance", MarketType::Spot)?;
    let filtered_markets = filter_active_usdt_pairs(all_spot_markets);
    
    println!("Активных USDT пар: {}", filtered_markets.len());
    
    // Сортируем по символу
    let mut sorted_markets = filtered_markets;
    sorted_markets.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    
    // Выводим первые 10
    for market in sorted_markets.iter().take(10) {
        println!("{}: {} / {}", market.symbol, market.base, market.quote);
    }
    
    Ok(())
}
```

### Анализ точности и лимитов

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let markets = fetch_markets("binance", MarketType::Spot)?;
    
    for market in &markets {
        if market.symbol == "BTCUSDT" {
            println!("=== Анализ рынка {} ===", market.symbol);
            println!("Активен: {}", market.active);
            println!("Маржинальная торговля: {}", market.margin);
            
            println!("\nТочность:");
            println!("  Минимальное изменение цены (tick_size): {}", market.precision.tick_size);
            println!("  Минимальное изменение количества (lot_size): {}", market.precision.lot_size);
            
            if let Some(limits) = &market.quantity_limit {
                println!("\nЛимиты количества:");
                println!("  Минимум: {:?}", limits.min);
                println!("  Максимум: {:?}", limits.max);
                println!("  Мин. нотиональная стоимость: {:?}", limits.notional_min);
                println!("  Макс. нотиональная стоимость: {:?}", limits.notional_max);
            }
            
            println!("\nКомиссии:");
            println!("  Maker: {}%", market.fees.maker * 100.0);
            println!("  Taker: {}%", market.fees.taker * 100.0);
            
            // Анализируем оригинальные данные от биржи
            if let Some(status) = market.info.get("status") {
                println!("\nСтатус от биржи: {}", status);
            }
            
            break;
        }
    }
    
    Ok(())
}
```

## Сравнение рынков

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Получаем данные по всем типам рынков для BTC
    let spot_markets = fetch_markets("binance", MarketType::Spot)?;
    let linear_swap_markets = fetch_markets("binance", MarketType::LinearSwap)?;
    let inverse_swap_markets = fetch_markets("binance", MarketType::InverseSwap)?;
    
    println!("=== Сравнение BTC рынков ===\n");
    
    // Спот
    if let Some(btc_spot) = spot_markets.iter().find(|m| m.symbol == "BTCUSDT") {
        println!("Спот (BTCUSDT):");
        println!("  Комиссия maker/taker: {}% / {}%", 
            btc_spot.fees.maker * 100.0, btc_spot.fees.taker * 100.0);
        println!("  Точность цены: {}", btc_spot.precision.tick_size);
    }
    
    // Linear Swap
    if let Some(btc_linear) = linear_swap_markets.iter().find(|m| m.symbol == "BTCUSDT") {
        println!("\nLinear Swap (BTCUSDT):");
        println!("  Комиссия maker/taker: {}% / {}%", 
            btc_linear.fees.maker * 100.0, btc_linear.fees.taker * 100.0);
        println!("  Точность цены: {}", btc_linear.precision.tick_size);
        println!("  Валюта расчетов: {:?}", btc_linear.settle);
        println!("  Стоимость контракта: {:?}", btc_linear.contract_value);
    }
    
    // Inverse Swap
    if let Some(btc_inverse) = inverse_swap_markets.iter().find(|m| m.base == "BTC") {
        println!("\nInverse Swap ({}):", btc_inverse.symbol);
        println!("  Комиссия maker/taker: {}% / {}%", 
            btc_inverse.fees.maker * 100.0, btc_inverse.fees.taker * 100.0);
        println!("  Точность цены: {}", btc_inverse.precision.tick_size);
        println!("  Валюта расчетов: {:?}", btc_inverse.settle);
        println!("  Стоимость контракта: {:?}", btc_inverse.contract_value);
    }
    
    Ok(())
}
```

## Обработка ошибок

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;

#[tokio::main]
async fn main() {
    match fetch_markets("binance", MarketType::Spot) {
        Ok(markets) => {
            println!("Успешно получено {} рынков", markets.len());
        }
        Err(e) => {
            eprintln!("Ошибка получения рынков: {}", e);
            // Возможные причины:
            // - Проблемы с сетью
            // - API Binance недоступно
            // - Изменился формат ответа API
            // - Превышены лимиты запросов
        }
    }
}
```

## Полный пример: Анализ всех рынков Binance

```rust
use crypto_markets::fetch_markets;
use crypto_market_type::MarketType;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Полный анализ рынков Binance ===\n");
    
    let market_types = vec![
        MarketType::Spot,
        MarketType::LinearSwap,
        MarketType::LinearFuture,
        MarketType::InverseSwap,
        MarketType::InverseFuture,
        MarketType::EuropeanOption,
    ];
    
    let mut total_markets = 0;
    let mut active_markets = 0;
    let mut base_currencies = HashMap::new();
    
    for market_type in market_types {
        match fetch_markets("binance", market_type) {
            Ok(markets) => {
                let active_count = markets.iter().filter(|m| m.active).count();
                
                println!("{:?}:", market_type);
                println!("  Всего рынков: {}", markets.len());
                println!("  Активных: {}", active_count);
                
                // Подсчитываем базовые валюты
                for market in &markets {
                    if market.active {
                        *base_currencies.entry(market.base.clone()).or_insert(0) += 1;
                    }
                }
                
                total_markets += markets.len();
                active_markets += active_count;
                
                // Показываем примеры символов
                let examples: Vec<_> = markets
                    .iter()
                    .filter(|m| m.active)
                    .take(3)
                    .map(|m| m.symbol.as_str())
                    .collect();
                println!("  Примеры: {:?}", examples);
                println!();
            }
            Err(e) => {
                println!("{:?}: Ошибка - {}", market_type, e);
            }
        }
    }
    
    println!("=== Общая статистика ===");
    println!("Всего рынков: {}", total_markets);
    println!("Активных рынков: {}", active_markets);
    
    // Топ-10 базовых валют
    let mut sorted_bases: Vec<_> = base_currencies.iter().collect();
    sorted_bases.sort_by(|a, b| b.1.cmp(a.1));
    
    println!("\nТоп-10 базовых валют:");
    for (base, count) in sorted_bases.iter().take(10) {
        println!("  {}: {} рынков", base, count);
    }
    
    Ok(())
}
```

## Полезные ссылки

- [Binance Spot API документация](https://binance-docs.github.io/apidocs/spot/en/)
- [Binance Futures API документация](https://binance-docs.github.io/apidocs/futures/en/)
- [Binance Delivery API документация](https://binance-docs.github.io/apidocs/delivery/en/)
- [Binance Options API документация](https://binance-docs.github.io/apidocs/voptions/en/)
- [Комиссии Binance](https://www.binance.com/en/fee/schedule)

## Примечания

1. **Кэширование:** Данные получаются в реальном времени, рекомендуется кэшировать результаты
2. **Лимиты API:** Соблюдайте лимиты запросов Binance API
3. **Обновления:** Информация о рынках может изменяться, регулярно обновляйте данные
4. **Фильтрация:** Всегда проверяйте поле `active` перед использованием рынка
5. **Точность:** Используйте поля `precision` для правильного форматирования цен и количеств
