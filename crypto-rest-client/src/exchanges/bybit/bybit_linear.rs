use super::super::utils::http_get;
use crate::error::Result;
use std::{collections::BTreeMap, time::Duration};
use serde_json::{json, Value};
const BASE_URL: &str = "https://api.bybit.com";

/// The RESTful client for Bybit.
///
/// Bybit has InverseSwap and LinearSwap markets.
///
/// * RESTful API doc: <https://bybit-exchange.github.io/docs/inverse/#t-marketdata>
/// * Trading at:
///     * InverseSwap <https://www.bybit.com/trade/inverse/>
///     * LinearSwap <https://www.bybit.com/trade/usdt/>
/// * Rate Limit: <https://bybit-exchange.github.io/docs/inverse/#t-ratelimits>
///   * GET method:
///     * 50 requests per second continuously for 2 minutes
///     * 70 requests per second continuously for 5 seconds
///   * POST method:
///     * 20 requests per second continuously for 2 minutes
///     * 50 requests per second continuously for 5 seconds
pub struct BybitRestClient {
    _api_key: Option<String>,
    _api_secret: Option<String>,
    _proxy: Option<String>,
}

impl BybitRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>, proxy: Option<String>) -> Self {
        BybitRestClient { _api_key: api_key, _api_secret: api_secret, _proxy: proxy }
    }

    pub async fn get_server_time(&self) -> Result<String> {
        let url = format!("{}/v5/market/time", BASE_URL);
        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .proxy(proxy)
            .build()?;

        let response = client.get(url).send().await?;
        let body: Value = response.json().await?;
        Ok(body["result"]["time"].as_str().unwrap_or_default().to_string())
    }

    pub async fn get_account_balance(&self, account_type: &str, coin: &str) -> Result<Vec<Value>> {
        // Проверка наличия прокси
        if self._proxy.is_none() {
            return Err(crate::error::Error("Прокси не указан".to_string()));
        }

        // Проверка API ключа и секрета
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(crate::error::Error("API ключ или секрет не указаны".to_string()));
        }

        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        
        // Для GET запросов, формируем строку запроса
        let query_string = format!("accountType={}&coin={}", account_type, coin);
        
        // Формируем строку для подписи (для GET запросов): {timestamp}{api_key}{recv_window}{query_string}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);
        
        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);
        
        // Конструируем URL с параметрами
        let url = format!("{}/v5/account/wallet-balance?{}", BASE_URL, query_string);
        
        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .proxy(proxy)
            .build()?;
        
        let response = client.get(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .send()
            .await?;
            
        // Проверяем статус ответа
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(crate::error::Error(format!(
                "Ошибка API Bybit: статус {}, ответ: {}", 
                status, 
                error_text
            )));
        }
        
        let body: Value = response.json().await?;
        
        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit: код {}, сообщение: {}", 
                    ret_code, 
                    ret_msg
                )));
            }
        }
        
        Ok(body["result"]["list"].as_array().unwrap_or(&Vec::new()).clone())
    }
    
    // Хелпер-функция для создания HMAC SHA256 подписи
    pub(crate) fn hmac_sha256(secret: String, message: String) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC может принимать ключ любой длины");
        mac.update(message.as_bytes());
        
        let result = mac.finalize();
        let bytes = result.into_bytes();
        
        hex::encode(bytes)
    }

    pub async fn create_order(&self, symbol: &str, side: &str, quantity: f64, price: f64, category: &str) -> Result<String> {
        // Проверка наличия прокси
        if self._proxy.is_none() {
            return Err(crate::error::Error("Прокси не указан".to_string()));
        }

        // Проверка API ключа и секрета
        if self._api_key.is_none() || self._api_secret.is_none() {
            return Err(crate::error::Error("API ключ или секрет не указаны".to_string()));
        }
        
        let api_key = self._api_key.clone().unwrap();
        let api_secret = self._api_secret.clone().unwrap();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";

        let order_body = json!({
            "category": category,
            "symbol": symbol,
            "side": side,
            "orderType": "Limit",
            "qty": quantity.to_string(),
            "price": price.to_string(),
            "timeInForce": "GTC",
        });
        
        // Для POST запросов, используем тело JSON
        let body_str = order_body.to_string();
        
        // Формируем строку для подписи (для POST запросов): {timestamp}{api_key}{recv_window}{body}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, body_str);
        
        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);
        
        // Конструируем URL 
        let url = format!("{}/v5/order/create", BASE_URL);
        
        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .proxy(proxy)
            .build()?;
        
        let response = client.post(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .json(&order_body)
            .send()
            .await?;
            
        // Проверяем статус ответа
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(crate::error::Error(format!(
                "Ошибка API Bybit: статус {}, ответ: {}", 
                status, 
                error_text
            )));
        }
        
        let body: Value = response.json().await?;
        
        // Отладочный вывод полного ответа
        log::debug!("Bybit API response: {}", body.to_string());
        
        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit: код {}, сообщение: {}", 
                    ret_code, 
                    ret_msg
                )));
            }
        }
        
        Ok(body["result"]["orderId"].as_str().unwrap_or_default().to_string())
    }

    /// Get the latest Level2 snapshot of orderbook.
    ///
    /// Top 50 bids and asks are returned.
    ///
    /// For example: <https://api.bybit.com/v2/public/orderBook/L2?symbol=BTCUSD>,
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        gen_api!(format!("/public/orderBook/L2?symbol={symbol}"))
    }

    /// Get open interest.
    ///
    /// For example:
    ///
    /// - <https://api.bybit.com/v2/public/open-interest?symbol=BTCUSD&period=5min&limit=200>
    /// - <https://api.bybit.com/v2/public/open-interest?symbol=BTCUSDT&period=5min&limit=200>
    /// - <https://api.bybit.com/v2/public/open-interest?symbol=BTCUSDU22&period=5min&limit=200>
    pub fn fetch_open_interest(symbol: &str) -> Result<String> {
        gen_api!(format!("/public/open-interest?symbol={symbol}&period=5min&limit=200"))
    }

    /// Get long-short ratio.
    ///
    /// For example:
    ///
    /// - <https://api.bybit.com/v2/public/account-ratio?symbol=BTCUSD&period=5min&limit=500>
    /// - <https://api.bybit.com/v2/public/account-ratio?symbol=BTCUSDT&period=5min&limit=500>
    /// - <https://api.bybit.com/v2/public/account-ratio?symbol=BTCUSDU22&period=5min&limit=500>
    pub fn fetch_long_short_ratio(symbol: &str) -> Result<String> {
        gen_api!(format!("/public/account-ratio?symbol={symbol}&period=5min&limit=200"))
    }

    pub async fn fetch_all_symbols() -> Result<Vec<Value>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let url = format!("{}/v5/market/tickers?category=linear", BASE_URL);
        let response = client.get(url).send().await?;
        let body: Value = response.json().await?;
        let mut markets = Vec::new();
        if let Some(data) = body["result"]["list"].as_array() {
            for market in data {
                markets.push(market.clone());
            }
        }
        Ok(markets)
    }
} 