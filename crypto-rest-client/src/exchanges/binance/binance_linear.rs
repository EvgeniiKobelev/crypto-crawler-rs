use super::{super::utils::{http_get, http_get_async, http_post_async}, utils::*};
use crate::error::Result;
use std::collections::BTreeMap;
use serde_json::Value;

const BASE_URL: &str = "https://fapi.binance.com";

/// Binance USDT-margined Future and Swap market.
///
/// * REST API doc: <https://binance-docs.github.io/apidocs/futures/en/>
/// * Trading at: <https://www.binance.com/en/futures/BTC_USDT>
/// * Rate Limits: <https://binance-docs.github.io/apidocs/futures/en/#limits>
///   * 2400 request weight per minute
pub struct BinanceLinearRestClient {
    api_key: Option<String>,
    api_secret: Option<String>,
    proxy: Option<String>,
}

impl BinanceLinearRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>, proxy: Option<String>) -> Self {
        BinanceLinearRestClient { 
            api_key, 
            api_secret,
            proxy,
        }
    }

    pub async fn get_account_balance(&self, asset: &str) -> Result<String> {
        let endpoint = format!("{}/fapi/v3/balance", BASE_URL);
        let mut params = BTreeMap::new();
        
        // Добавляем recvWindow для Binance API
        params.insert("recvWindow".to_string(), "5000".to_string());
        
        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        ).await?;
        
        let json: Value = serde_json::from_str(&response)?;
        if let Some(balances) = json.as_array() {
            for balance in balances {
                if balance["asset"].as_str() == Some(asset) {
                    if let Some(balance_str) = balance["balance"].as_str() {
                        return Ok(balance_str.to_string());
                    } else if let Some(balance_num) = balance["balance"].as_f64() {
                        return Ok(balance_num.to_string());
                    }
                }
            }
        }
        Ok("0".to_string())
    }

    pub async fn create_order(&self, symbol: &str, side: &str, mut quantity: f64, price: f64, market_type: &str) -> Result<String> {
        let mut params = BTreeMap::new();
        
        // Преобразуем символ в верхний регистр и убираем пробелы
        let normalized_symbol = symbol.trim().to_uppercase();
        params.insert("symbol".to_string(), normalized_symbol.clone());
        
        // Проверка стороны (всегда в верхнем регистре)
        let normalized_side = match side.trim().to_uppercase().as_str() {
            "BUY" => "BUY",
            "SELL" => "SELL",
            _ => {
                return Err(crate::error::Error(
                    format!("Неверное значение стороны: {}", side)
                ));
            }
        };
        params.insert("side".to_string(), normalized_side.to_string());
        
        // Устанавливаем тип и timeInForce как в успешном запросе
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("timeInForce".to_string(), "GTC".to_string());
        
        // LOKAUSDT требует целочисленное количество
        if normalized_symbol == "LOKAUSDT" {
            // Округляем до целого числа
            let rounded_quantity = quantity.floor();
            params.insert("quantity".to_string(), format!("{:.0}", rounded_quantity));
            
            // Форматируем цену с 4 знаками после запятой
            let rounded_price = (price * 10000.0).round() / 10000.0;
            params.insert("price".to_string(), format!("{:.4}", rounded_price));
        } else {
            // Для других пар используем прежнюю логику
            let (quantity_precision, price_precision, min_qty, step_size) = match normalized_symbol.as_str() {
                "BTCUSDT" => (3, 1, 0.001, 0.001),
                "ETHUSDT" => (3, 2, 0.001, 0.001),
                _ => (3, 4, 0.001, 0.001),
            };
            
            // Проверка минимального количества
            if quantity < min_qty {
                quantity = min_qty;
            }
            
            // Округляем до ближайшего кратного шагу объема
            let rounded_quantity = (quantity / step_size).floor() * step_size;
            
            // Форматируем количество с правильной точностью
            let formatted_quantity = format!("{:.*}", quantity_precision, rounded_quantity);
            
            // Округляем цену до нужной точности
            let price_factor = 10.0_f64.powi(price_precision as i32);
            let rounded_price = (price * price_factor).round() / price_factor;
            let formatted_price = format!("{:.*}", price_precision, rounded_price);
            
            params.insert("quantity".to_string(), formatted_quantity);
            params.insert("price".to_string(), formatted_price);
        }
        
        // Увеличиваем recvWindow для предотвращения ошибок подписи
        params.insert("recvWindow".to_string(), "10000".to_string());
        
        // URL точно как в успешном запросе
        let endpoint = format!("{}/fapi/v1/order", BASE_URL);
        
        match http_post_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        ).await {
            Ok(response) => Ok(response),
            Err(e) => Err(e)
        }
    }

    /// Get compressed, aggregate trades.
    ///
    /// Equivalent to `/fapi/v1/aggTrades` with `limit=1000`
    ///
    /// For example:
    ///
    /// - <https://fapi.binance.com/fapi/v1/aggTrades?symbol=BTCUSDT&limit=1000>
    /// - <https://fapi.binance.com/fapi/v1/aggTrades?symbol=BTCUSDT_210625&limit=1000>
    pub fn fetch_agg_trades(
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Result<String> {
        check_symbol(symbol);
        let symbol = Some(symbol);
        let limit = Some(1000);
        gen_api_binance!("/fapi/v1/aggTrades", symbol, from_id, start_time, end_time, limit)
    }

    /// Get a Level2 snapshot of orderbook.
    ///
    /// Equivalent to `/fapi/v1/depth` with `limit=1000`
    ///
    /// For example:
    ///
    /// - <https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT&limit=1000>
    /// - <https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT_211231&limit=1000>
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        check_symbol(symbol);
        let symbol = Some(symbol);
        let limit = Some(1000);
        gen_api_binance!("/fapi/v1/depth", symbol, limit)
    }

    /// Get open interest.
    ///
    /// For example:
    ///
    /// - <https://fapi.binance.com/fapi/v1/openInterest?symbol=BTCUSDT>
    /// - <https://fapi.binance.com/fapi/v1/openInterest?symbol=BTCUSDT_211231>
    pub fn fetch_open_interest(symbol: &str) -> Result<String> {
        check_symbol(symbol);
        let symbol = Some(symbol);
        gen_api_binance!("/fapi/v1/openInterest", symbol)
    }
}
