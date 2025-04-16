use super::super::utils::http_get;
use crate::error::{Error, Result};
use base64;
use hmac::{Hmac, Mac};
use serde_json::Value;
use serde_json::json;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::time::Duration;

const BASE_URL: &str = "https://api.bitget.com";

/// The RESTful client for Bitget swap markets.
///
/// * RESTful API doc: <https://bitgetlimited.github.io/apidoc/en/mix/#restapi>
/// * Trading at: <https://www.bitget.com/mix/>
pub struct BitgetSwapRestClient {
    _api_key: Option<String>,
    _api_secret: Option<String>,
    _api_passphrase: Option<String>,
    _proxy: Option<String>,
}

impl BitgetSwapRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>) -> Self {
        BitgetSwapRestClient {
            _api_key: api_key,
            _api_secret: api_secret,
            _api_passphrase: None,
            _proxy: None,
        }
    }

    /// Создает новый клиент Bitget Swap REST с API ключом, секретом, паролем и прокси.
    ///
    /// * `api_key` - API ключ
    /// * `api_secret` - API секрет
    /// * `api_passphrase` - Пароль (passphrase) от API
    /// * `proxy` - Строка прокси в формате "http://username:password@host:port" или "socks5://username:password@host:port"
    pub fn new_with_credentials(
        api_key: Option<String>,
        api_secret: Option<String>,
        api_passphrase: Option<String>,
        proxy: Option<String>,
    ) -> Self {
        BitgetSwapRestClient {
            _api_key: api_key,
            _api_secret: api_secret,
            _api_passphrase: api_passphrase,
            _proxy: proxy,
        }
    }

    /// Get the latest Level2 snapshot of orderbook.
    ///
    /// For example: <https://api.bitget.com/api/mix/v1/market/depth?symbol=BTCUSDT_UMCBL&limit=100>
    ///
    /// Rate Limit：20 requests per 2 seconds
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/mix/v1/market/depth?symbol={symbol}&limit=100"))
    }

    /// Get open interest.
    ///
    /// For example:
    ///
    /// - <https://api.bitget.com/api/mix/v1/market/open-interest?symbol=BTCUSDT_UMCBL>
    pub fn fetch_open_interest(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/mix/v1/market/open-interest?symbol={symbol}"))
    }

    /// Get ticker information for all symbols.
    ///
    /// For example: <https://api.bitget.com/api/mix/v1/market/tickers?productType=umcbl>
    ///
    /// Rate Limit: 20 requests per second (IP)
    ///
    /// Returns information including:
    /// - symbol: Symbol Id
    /// - last: Latest price
    /// - bestAsk/bestBid: Ask/Bid prices
    /// - high24h/low24h: Highest/Lowest price in 24 hours
    /// - baseVolume: Base currency trading volume
    /// - quoteVolume: Quote currency trading volume
    /// - fundingRate: Funding rate
    /// - holdingAmount: Holding amount
    pub async fn fetch_all_symbols() -> Result<Vec<Value>> {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;

        let usdt_futures_url =
            "https://api.bitget.com/api/mix/v1/market/contracts?productType=umcbl";
        let usdt_futures_response = client.get(usdt_futures_url).send().await?;
        let usdt_futures_json: Value = usdt_futures_response.json().await?;

        let mut contracts = Vec::new();
        if let Some(data) = usdt_futures_json["data"].as_array() {
            for contract in data {
                contracts.push(contract.clone());
            }
        }

        Ok(contracts)
    }

    /// Create a new order.
    ///
    /// * `symbol` - The trading pair, e.g., "BTCUSDT_UMCBL"
    /// * `margin_coin` - The margin coin, e.g., "USDT"
    /// * `side` - For USD-M Futures: "open_long", "open_short", "close_long", "close_short"
    /// * `order_type` - "limit", "market", "post_only", etc.
    /// * `size` - The amount of contracts to trade
    /// * `price` - The price for a limit order (None for market orders)
    /// * `client_order_id` - Optional client order ID
    ///
    /// Returns the order ID if successful.
    ///
    /// API documentation: <https://bitgetlimited.github.io/apidoc/en/mix/#place-order>
    pub async fn create_order(
        &self,
        symbol: &str,
        margin_coin: &str,
        side: &str,
        order_type: &str,
        size: f64,
        price: Option<f64>,
        client_order_id: Option<&str>,
    ) -> Result<String> {
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(Error("API key and secret are required for placing orders".to_string()));
        }

        if self._api_passphrase.is_none() {
            return Err(Error("API passphrase is required for placing orders".to_string()));
        }

        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        let api_passphrase = self._api_passphrase.clone().unwrap();

        let mut order_data = json!({
            "symbol": symbol,
            "marginCoin": margin_coin,
            "side": side,
            "orderType": order_type,
            "size": size.to_string(),
            "timeInForceValue": "normal",
        });

        if let Some(p) = price {
            order_data["price"] = json!(p.to_string());
        }

        if let Some(id) = client_order_id {
            order_data["clientOid"] = json!(id);
        }

        let body = order_data.to_string();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let request_path = "/api/mix/v1/order/placeOrder";

        // Строка для подписи: timestamp + method.toUpperCase() + requestPath + body
        let sign_payload = format!("{}POST{}{}", timestamp, request_path, body);

        // Создаем HMAC-SHA256 подпись
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|_| Error("HMAC error".to_string()))?;
        mac.update(sign_payload.as_bytes());
        let signature = base64::encode(mac.finalize().into_bytes());

        // Настраиваем клиент с учетом прокси, если оно указано
        let client_builder = reqwest::Client::builder().timeout(Duration::from_secs(10));

        let client_builder = if let Some(proxy_url) = &self._proxy {
            if proxy_url.starts_with("http://") || proxy_url.starts_with("https://") {
                client_builder.proxy(reqwest::Proxy::http(proxy_url)?)
            } else if proxy_url.starts_with("socks5://") {
                client_builder.proxy(reqwest::Proxy::all(proxy_url)?)
            } else {
                client_builder
            }
        } else {
            client_builder
        };

        let client = client_builder.build()?;

        let url = format!("{}{}", BASE_URL, request_path);
        let response = client
            .post(&url)
            .header("ACCESS-KEY", &api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &api_passphrase)
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

    /// Cancel an order.
    ///
    /// * `symbol` - The trading pair, e.g., "BTCUSDT_UMCBL"
    /// * `order_id` - The order ID to cancel
    /// * `margin_coin` - The margin coin, e.g., "USDT"
    ///
    /// Returns success or error.
    ///
    /// API documentation: <https://bitgetlimited.github.io/apidoc/en/mix/#cancel-order>
    pub async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
        margin_coin: &str,
    ) -> Result<bool> {
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(Error("API key and secret are required for canceling orders".to_string()));
        }

        if self._api_passphrase.is_none() {
            return Err(Error("API passphrase is required for canceling orders".to_string()));
        }

        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        let api_passphrase = self._api_passphrase.clone().unwrap();

        let cancel_data = json!({
            "symbol": symbol,
            "orderId": order_id,
            "marginCoin": margin_coin,
        });

        let body = cancel_data.to_string();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let request_path = "/api/mix/v1/order/cancel-order";

        // Строка для подписи: timestamp + method.toUpperCase() + requestPath + body
        let sign_payload = format!("{}POST{}{}", timestamp, request_path, body);

        // Создаем HMAC-SHA256 подпись
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|_| Error("HMAC error".to_string()))?;
        mac.update(sign_payload.as_bytes());
        let signature = base64::encode(mac.finalize().into_bytes());

        // Настраиваем клиент с учетом прокси, если оно указано
        let client_builder = reqwest::Client::builder().timeout(Duration::from_secs(10));

        let client_builder = if let Some(proxy_url) = &self._proxy {
            if proxy_url.starts_with("http://") || proxy_url.starts_with("https://") {
                client_builder.proxy(reqwest::Proxy::http(proxy_url)?)
            } else if proxy_url.starts_with("socks5://") {
                client_builder.proxy(reqwest::Proxy::all(proxy_url)?)
            } else {
                client_builder
            }
        } else {
            client_builder
        };

        let client = client_builder.build()?;

        let url = format!("{}{}", BASE_URL, request_path);
        let response = client
            .post(&url)
            .header("ACCESS-KEY", &api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &api_passphrase)
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

        Ok(true)
    }

    /// Устанавливает плечо (leverage) для символа.
    ///
    /// * `symbol` - Торговая пара, например, "btcusdt"
    /// * `product_type` - Тип продукта, например, "USDT-FUTURES"
    /// * `margin_coin` - Валюта маржи, например, "usdt"
    /// * `leverage` - Значение плеча, например, "20"
    ///
    /// Возвращает успех или ошибку.
    ///
    /// API документация: <https://bitgetlimited.github.io/apidoc/en/mix/#set-leverage>
    pub async fn set_leverage(
        &self,
        symbol: &str,
        product_type: &str,
        margin_coin: &str,
        leverage: &str,
    ) -> Result<bool> {
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(Error("API key and secret are required for setting leverage".to_string()));
        }

        if self._api_passphrase.is_none() {
            return Err(Error("API passphrase is required for setting leverage".to_string()));
        }

        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        let api_passphrase = self._api_passphrase.clone().unwrap();

        let leverage_data = json!({
            "symbol": symbol,
            "productType": product_type,
            "marginCoin": margin_coin,
            "leverage": leverage,
        });

        let body = leverage_data.to_string();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let request_path = "/api/v2/mix/account/set-leverage";

        // Строка для подписи: timestamp + method.toUpperCase() + requestPath + body
        let sign_payload = format!("{}POST{}{}", timestamp, request_path, body);

        // Создаем HMAC-SHA256 подпись
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|_| Error("HMAC error".to_string()))?;
        mac.update(sign_payload.as_bytes());
        let signature = base64::encode(mac.finalize().into_bytes());

        // Настраиваем клиент с учетом прокси, если оно указано
        let client_builder = reqwest::Client::builder().timeout(Duration::from_secs(10));

        let client_builder = if let Some(proxy_url) = &self._proxy {
            if proxy_url.starts_with("http://") || proxy_url.starts_with("https://") {
                client_builder.proxy(reqwest::Proxy::http(proxy_url)?)
            } else if proxy_url.starts_with("socks5://") {
                client_builder.proxy(reqwest::Proxy::all(proxy_url)?)
            } else {
                client_builder
            }
        } else {
            client_builder
        };

        let client = client_builder.build()?;

        let url = format!("{}{}", BASE_URL, request_path);
        let response = client
            .post(&url)
            .header("ACCESS-KEY", &api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &api_passphrase)
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

        Ok(true)
    }
}
