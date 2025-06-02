use super::super::utils::{http_get, http_get_async, http_post_async};
use crate::error::Result;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://open-api.bingx.com";

/// BingX Spot market.
///
/// * RESTful API doc: <https://bingx-api.github.io/docs/spot-api-v1.html>
/// * Trading at: <https://bingx.com/en-us/spot/BTCUSDT/>
/// * Rate Limits: <https://bingx-api.github.io/docs/spot-api-v1.html#request-rate-limit>
///   * 1200 request weight per minute for each IP
pub struct BingxSpotRestClient {
    api_key: Option<String>,
    api_secret: Option<String>,
    proxy: Option<String>,
}

impl BingxSpotRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>, proxy: Option<String>) -> Self {
        BingxSpotRestClient { api_key, api_secret, proxy }
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

    pub async fn get_account_balance(&self, asset: Option<&str>) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/spot/v1/account/balance", BASE_URL);
        let mut params = BTreeMap::new();
        let timestamp = Self::get_timestamp().to_string();
        params.insert("timestamp".to_string(), timestamp);

        if let Some(coin) = asset {
            params.insert("coin".to_string(), coin.to_string());
        }

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
        quantity: f64,
        price: Option<f64>,
        order_type: &str,
    ) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/spot/v1/trade/order", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), side.to_string());
        params.insert("type".to_string(), order_type.to_string());
        params.insert("quantity".to_string(), quantity.to_string());

        if let Some(p) = price {
            params.insert("price".to_string(), p.to_string());
        }

        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

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

    pub async fn get_order_status(
        &self,
        symbol: &str,
        order_id: Option<String>,
        client_order_id: Option<String>,
    ) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        if order_id.is_none() && client_order_id.is_none() {
            return Err(crate::error::Error(
                "Either order_id or client_order_id must be provided".to_string(),
            ));
        }

        let endpoint = format!("{}/openApi/spot/v1/trade/query", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());

        if let Some(id) = order_id {
            params.insert("orderId".to_string(), id);
        }

        if let Some(id) = client_order_id {
            params.insert("origClientOrderId".to_string(), id);
        }

        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

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

    /// Отменить существующий ордер.
    ///
    /// Использует BingX Spot API v1 эндпоинт `/openApi/spot/v1/trade/cancel` для отмены ордера.
    /// Требует API ключ и секретный ключ для аутентификации.
    ///
    /// # Параметры
    /// * `symbol` - Торговая пара в формате "BTC-USDT" (с дефисом)
    /// * `order_id` - Идентификатор ордера для отмены
    ///
    /// # Возвращает
    /// * `Result<String>` - JSON ответ с информацией об отмененном ордере
    ///
    /// # Ошибки
    /// * `Error` - Если отсутствуют API ключи
    /// * `Error` - Если ордер не найден или уже отменен
    /// * `Error` - Если ордер нельзя отменить (например, уже исполнен)
    ///
    /// # Пример
    /// ```
    /// let client = BingxSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// let result = client.cancel_order("BTC-USDT", "12345678").await?;
    /// ```
    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<String> {
        if self.api_key.is_none() || self.api_secret.is_none() {
            return Err(crate::error::Error("API key and secret are required".to_string()));
        }

        let endpoint = format!("{}/openApi/spot/v1/trade/cancel", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("orderId".to_string(), order_id.to_string());
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

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

    /// Get a Level2 snapshot of orderbook.
    ///
    /// For example: <https://open-api.bingx.com/openApi/spot/v1/market/depth?symbol=BTC-USDT&limit=100>
    pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        let symbol = symbol.replace('/', "-");
        let url = format!("{}/openApi/spot/v1/market/depth", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol);
        params.insert("limit".to_string(), "100".to_string());

        http_get_async(&url, &mut params, None, None, None).await
    }

    /// Get all available trading pairs
    pub async fn fetch_all_symbols() -> Result<Vec<Value>> {
        let url = format!("{}/openApi/spot/v1/common/symbols", BASE_URL);
        let params = BTreeMap::new();

        let resp = http_get(&url, &params)?;
        let json: Value = serde_json::from_str(&resp)?;

        if let Some(data) = json["data"].as_array() { Ok(data.clone()) } else { Ok(Vec::new()) }
    }
}
