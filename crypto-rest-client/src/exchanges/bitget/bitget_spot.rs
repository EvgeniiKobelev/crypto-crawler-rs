use super::super::utils::http_get;
use crate::error::{Error, Result};
use std::collections::BTreeMap;
use serde_json::json;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64;

const BASE_URL: &str = "https://api.bitget.com";

/// The RESTful client for Bitget spot market.
///
/// * RESTful API doc: <https://bitgetlimited.github.io/apidoc/en/spot/>
/// * Trading at: <https://www.bitget.com/spot/>
pub struct BitgetSpotRestClient {
    _api_key: Option<String>,
    _api_secret: Option<String>,
}

impl BitgetSpotRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>) -> Self {
        BitgetSpotRestClient { _api_key: api_key, _api_secret: api_secret }
    }

    /// Get the latest Level2 snapshot of orderbook.
    ///
    /// Top 150 bids and asks are returned.
    ///
    /// For example: <https://api.bitget.com/api/spot/v1/market/depth?symbol=BTCUSDT_SPBL&type=step0>,
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/spot/v1/market/depth?symbol={symbol}&type=step0"))
    }

    /// Create a new order.
    ///
    /// * `symbol` - The trading pair, e.g., "BTCUSDT_SPBL"
    /// * `side` - "buy" or "sell"
    /// * `order_type` - "limit" or "market"
    /// * `quantity` - The amount of base currency to trade
    /// * `price` - The price for a limit order (None for market orders)
    /// * `client_order_id` - Optional client order ID
    /// * `force` - Order type: "normal", "post_only", "fok", "ioc" (default: "normal")
    ///
    /// Returns the order ID if successful.
    ///
    /// API documentation: <https://bitgetlimited.github.io/apidoc/en/spot/#place-order>
    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: f64,
        price: Option<f64>,
        client_order_id: Option<&str>,
        force: Option<&str>,
    ) -> Result<String> {
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(Error("API key and secret are required for placing orders".to_string()));
        }

        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        
        let mut order_data = json!({
            "symbol": symbol,
            "side": side.to_lowercase(),
            "orderType": order_type.to_lowercase(),
            "quantity": quantity.to_string(),
            "force": force.unwrap_or("normal"),
        });

        if let Some(p) = price {
            order_data["price"] = json!(p.to_string());
        }

        if let Some(id) = client_order_id {
            order_data["clientOrderId"] = json!(id);
        }
        
        let body = order_data.to_string();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let request_path = "/api/spot/v1/trade/orders";
        
        // Строка для подписи: timestamp + method.toUpperCase() + requestPath + body
        let sign_payload = format!("{}POST{}{}", timestamp, request_path, body);
        
        // Создаем HMAC-SHA256 подпись
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|_| Error("HMAC error".to_string()))?;
        mac.update(sign_payload.as_bytes());
        let signature = base64::encode(mac.finalize().into_bytes());

        // Отправляем запрос
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        
        let url = format!("{}{}", BASE_URL, request_path);
        let response = client.post(&url)
            .header("ACCESS-KEY", &api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", "") // Если в API нужен passphrase, здесь нужно его добавить
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(Error(format!("Bitget API error: {}", error_text)));
        }
        
        let response_body: serde_json::Value = response.json().await?;
        
        if response_body["code"].as_str().unwrap_or("") != "00000" {
            return Err(Error(format!(
                "Bitget API error: {}", 
                response_body["msg"].as_str().unwrap_or("Unknown error")
            )));
        }
        
        Ok(response_body["data"]["orderId"].as_str().unwrap_or_default().to_string())
    }
}
