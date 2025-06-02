use super::super::utils::http_get;
use crate::{error::Result, exchanges::utils::http_get_async};
use std::collections::BTreeMap;

const BASE_URL: &str = "https://contract.mexc.com";

/// MEXC Swap market.
///
/// * REST API doc: <https://mxcdevelop.github.io/APIDoc/>
/// * Trading at: <https://contract.mexc.com/exchange>
pub struct MexcSwapRestClient {
    _api_key: Option<String>,
    _api_secret: Option<String>,
}

impl MexcSwapRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>) -> Self {
        MexcSwapRestClient { _api_key: api_key, _api_secret: api_secret }
    }

    /// Get most recent trades.
    ///
    /// For example: <https://contract.mexc.com/api/v1/contract/deals/BTC_USDT>
    pub fn fetch_trades(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/v1/contract/deals/{symbol}"))
    }

    /// Get the latest Level2 snapshot of orderbook.
    ///
    /// Top 2000 bids and asks will be returned.
    ///
    /// For example: <https://contract.mexc.com/api/v1/contract/depth/BTC_USDT?limit=2000>
    ///
    /// Rate limit: 20 times /2 seconds
    pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        let endpoint = format!("{}/api/v1/contract/depth", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("limit".to_string(), "2000".to_string());

        http_get_async(&endpoint, &mut params, None, None, None).await
    }

    /// Получить listen_key для WebSocket приватных данных (Swap API).
    ///
    /// # Примечания
    /// - MEXC Swap может использовать другой API эндпоинт для listen_key
    /// - Требует дальнейшего исследования документации MEXC Swap API
    pub async fn get_listen_key(&self) -> Result<String> {
        Err(crate::error::Error(
            "get_listen_key для MEXC Swap пока не реализован - требуется исследование API"
                .to_string(),
        ))
    }
}
