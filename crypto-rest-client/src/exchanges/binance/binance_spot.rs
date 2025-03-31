use super::{super::utils::{http_get, http_get_async}, utils::*};
use crate::error::Result;
use std::collections::BTreeMap;
use serde_json::Value;

const BASE_URL: &str = "https://api.binance.com";

/// Binance Spot market.
///
/// * RESTful API doc: <https://binance-docs.github.io/apidocs/spot/en/>
/// * Trading at: <https://www.binance.com/en/trade/BTC_USDT>
/// * Rate Limits: <https://binance-docs.github.io/apidocs/spot/en/#limits>
///   * 1200 request weight per minute
///   * 6100 raw requests per 5 minutes
pub struct BinanceSpotRestClient {
    api_key: Option<String>,
    api_secret: Option<String>,
    proxy: Option<String>,
}

impl BinanceSpotRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>, proxy: Option<String>) -> Self {
        BinanceSpotRestClient { 
            api_key, 
            api_secret,
            proxy,
        }
    }

    pub async fn get_account_balance(&self, asset: &str) -> Result<String> {
        let endpoint = format!("{}/api/v3/account", BASE_URL);
        let mut params = BTreeMap::new();
        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        ).await?;
        
        let json: Value = serde_json::from_str(&response)?;
        let balances = json["balances"].as_array().unwrap();
        for balance in balances {
            if balance["asset"].as_str() == Some(asset) {
                return Ok(balance["free"].as_str().unwrap_or("0").to_string());
            }
        }
        Ok("0".to_string())
    }

    pub async fn create_order(&self, symbol: &str, side: &str, quantity: f64, price: f64, market_type: &str) -> Result<String> {
        let endpoint = format!("{}/api/v3/order", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), side.to_string());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("quantity".to_string(), quantity.to_string());
        params.insert("price".to_string(), price.to_string());
        params.insert("timeInForce".to_string(), "GTC".to_string());

        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        ).await?;
        
        Ok(response)
    }

    /// Create a market order.
    ///
    /// * `symbol` - Trading pair, e.g., BTCUSDT
    /// * `side` - Order side: BUY or SELL
    /// * `quantity` - Order quantity
    ///
    /// For example: Create a market order to buy 0.1 BTC with USDT
    pub async fn create_market_order(&self, symbol: &str, side: &str, quantity: f64) -> Result<String> {
        let endpoint = format!("{}/api/v3/order", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), side.to_string());
        params.insert("type".to_string(), "MARKET".to_string());
        params.insert("quantity".to_string(), quantity.to_string());

        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        ).await?;
        
        Ok(response)
    }

    
    /// Get compressed, aggregate trades.
    ///
    /// Equivalent to `/api/v3/aggTrades` with `limit=1000`
    ///
    /// For example: <https://api.binance.com/api/v3/aggTrades?symbol=BTCUSDT&limit=1000>
    pub fn fetch_agg_trades(
        symbol: &str,
        from_id: Option<u64>,
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Result<String> {
        check_symbol(symbol);
        let symbol = Some(symbol);
        let limit = Some(1000);
        gen_api_binance!("/api/v3/aggTrades", symbol, from_id, start_time, end_time, limit)
    }

    /// Get a Level2 snapshot of orderbook.
    ///
    /// Equivalent to `/api/v3/depth` with `limit=1000`
    ///
    /// For example: <https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=1000>
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        check_symbol(symbol);
        let symbol = Some(symbol);
        let limit = Some(1000);
        gen_api_binance!("/api/v3/depth", symbol, limit)
    }
}
