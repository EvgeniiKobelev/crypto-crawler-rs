use super::super::utils::{http_get, http_get_async, http_post_async, http_request_async};
use crate::error::Result;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://open-api.bingx.com";

/// BingX Swap market (Perpetual contracts).
///
/// * RESTful API doc: <https://bingx-api.github.io/docs/swap-api.html>
/// * Trading at: <https://bingx.com/en-us/futures/BTCUSDT/>
/// * Rate Limits: <https://bingx-api.github.io/docs/swap-api.html#rate-limit>
///   * 1200 request weight per minute for each IP
pub struct BingxSwapRestClient {
    api_key: Option<String>,
    api_secret: Option<String>,
    proxy: Option<String>,
}

impl BingxSwapRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>, proxy: Option<String>) -> Self {
        BingxSwapRestClient { api_key, api_secret, proxy }
    }

    fn get_timestamp() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    fn sign_request(secret: &str, payload: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    pub async fn get_account_balance(&self) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/swap/v2/user/balance", BASE_URL);
        let mut params = BTreeMap::new();
        let timestamp = Self::get_timestamp().to_string();
        params.insert("timestamp".to_string(), timestamp);

        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        )
        .await?;

        Ok(response)
    }

    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        position_side: &str,
        quantity: f64,
        price: Option<f64>,
        order_type: &str,
    ) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/swap/v2/trade/order", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), side.to_uppercase().to_string());
        params.insert("positionSide".to_string(), position_side.to_string());
        params.insert("type".to_string(), order_type.to_uppercase().to_string());
        params.insert("quantity".to_string(), quantity.to_string());

        if let Some(p) = price {
            params.insert("price".to_string(), p.to_string());
        }

        // Для API v2 вместо timestamp используется timestamp_m
        params.insert("timestamp_m".to_string(), Self::get_timestamp().to_string());

        let response = http_post_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        )
        .await?;

        Ok(response)
    }

    pub async fn get_position(&self, symbol: Option<&str>) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/swap/v2/user/positions", BASE_URL);
        let mut params = BTreeMap::new();

        if let Some(s) = symbol {
            params.insert("symbol".to_string(), s.to_string());
        }

        params.insert("timestamp_m".to_string(), Self::get_timestamp().to_string());

        let response = http_get_async(
            &endpoint,
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        )
        .await?;

        Ok(response)
    }

    /// Get a Level2 snapshot of orderbook.
    ///
    /// For example: <https://open-api.bingx.com/openApi/swap/v2/quote/depth?symbol=BTC-USDT&limit=100>
    pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        let symbol = symbol.replace('/', "-");
        let url = format!("{}/openApi/swap/v2/quote/depth", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol);
        params.insert("limit".to_string(), "100".to_string());

        http_get_async(&url, &mut params, None, None, None).await
    }

    /// Get open interest data for a specific symbol
    ///
    /// For example: <https://open-api.bingx.com/openApi/swap/v2/quote/openInterest?symbol=BTC-USDT>
    pub async fn fetch_open_interest(symbol: &str) -> Result<String> {
        let symbol = symbol.replace('/', "-");
        let url = format!("{}/openApi/swap/v2/quote/openInterest", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol);

        http_get(&url, &params)
    }

    /// Get all available contract symbols
    pub async fn fetch_all_symbols() -> Result<Vec<Value>> {
        let url = format!("{}/openApi/swap/v2/quote/contracts", BASE_URL);
        let params = BTreeMap::new();

        let resp = http_get(&url, &params)?;
        let json: Value = serde_json::from_str(&resp)?;

        if let Some(data) = json["data"].as_array() { Ok(data.clone()) } else { Ok(Vec::new()) }
    }

    /// Отменяет существующий ордер
    ///
    /// # Аргументы
    ///
    /// * `symbol` - Торговая пара, например "BTC-USDT"
    /// * `order_id` - Идентификатор ордера, который требуется отменить
    ///
    /// # Возвращаемое значение
    ///
    /// Возвращает JSON строку с ответом API или ошибку
    ///
    /// # Пример
    ///
    /// ```
    /// use crypto_rest_client::exchanges::bingx::BingxSwapRestClient;
    ///
    /// async fn example() {
    ///     let client = BingxSwapRestClient::new(
    ///         Some("api_key".to_string()),
    ///         Some("secret_key".to_string()),
    ///         None,
    ///     );
    ///     let response = client.cancel_order("BTC-USDT", "123456").await.unwrap();
    ///     println!("{}", response);
    /// }
    /// ```
    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/swap/v2/trade/order", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("orderId".to_string(), order_id.to_string());
        params.insert("timestamp_m".to_string(), Self::get_timestamp().to_string());

        // Для отмены ордера используется DELETE запрос
        let response = http_request_async(
            &endpoint,
            "DELETE",
            &mut params,
            self.api_key.as_deref(),
            self.api_secret.as_deref(),
            self.proxy.as_deref(),
        )
        .await?;

        Ok(response)
    }
}
