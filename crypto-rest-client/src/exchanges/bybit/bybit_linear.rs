use super::super::utils::http_get;
use crate::error::Result;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
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
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(10)).proxy(proxy).build()?;

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
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .get(&url)
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
                status, error_text
            )));
        }

        let body: Value = response.json().await?;

        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit: код {}, сообщение: {}",
                    ret_code, ret_msg
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

    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
        category: &str,
    ) -> Result<String> {
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
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .post(&url)
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
                status, error_text
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
                    ret_code, ret_msg
                )));
            }
        }

        Ok(body["result"]["orderId"].as_str().unwrap_or_default().to_string())
    }

    pub async fn cancel_order(
        &self,
        category: &str,
        symbol: &str,
        order_id: &str,
    ) -> Result<String> {
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

        let cancel_order_body = json!({
            "category": category,
            "symbol": symbol,
            "orderId": order_id,
        });

        // Для POST запросов, используем тело JSON
        let body_str = cancel_order_body.to_string();

        // Формируем строку для подписи (для POST запросов): {timestamp}{api_key}{recv_window}{body}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, body_str);

        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);

        // Конструируем URL
        let url = format!("{}/v5/order/cancel", BASE_URL);

        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .post(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .json(&cancel_order_body)
            .send()
            .await?;

        // Проверяем статус ответа
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(crate::error::Error(format!(
                "Ошибка API Bybit при отмене ордера: статус {}, ответ: {}",
                status, error_text
            )));
        }

        let body: Value = response.json().await?;

        // Отладочный вывод полного ответа
        log::debug!("Bybit API cancel order response: {}", body.to_string());

        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка API Bybit");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit при отмене ордера: код {}, сообщение: {}",
                    ret_code, ret_msg
                )));
            }
        }

        // Ожидаем, что API вернет ID отмененного ордера в поле result.orderId
        Ok(body["result"]["orderId"].as_str().unwrap_or_default().to_string())
    }

    pub async fn get_positions(
        &self,
        category: &str,
        symbol: Option<&str>,
        settle_coin: Option<&str>,
    ) -> Result<Vec<Value>> {
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

        let mut params = vec![format!("category={}", category)];
        if let Some(symbol) = symbol {
            if !symbol.is_empty() {
                params.push(format!("symbol={}", symbol));
            }
        }
        if let Some(coin) = settle_coin {
            if !coin.is_empty() {
                params.push(format!("settleCoin={}", coin));
            }
        }

        if params.len() == 1 {
            // Только category — ошибка, нужен хотя бы symbol или settleCoin
            return Err(crate::error::Error(
                "Необходимо указать symbol или settleCoin".to_string(),
            ));
        }

        let query_string = params.join("&");

        // Формируем строку для подписи (для GET запросов): {timestamp}{api_key}{recv_window}{query_string}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);

        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);

        // Конструируем URL с параметрами
        let url = format!("{}/v5/position/list?{}", BASE_URL, query_string);

        let proxy_url = self._proxy.clone().unwrap();
        let proxy = reqwest::Proxy::http(&proxy_url)
            .map_err(|e| crate::error::Error(format!("Ошибка создания прокси: {}", e)))?;
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .get(&url)
            .header("X-BAPI-API-KEY", api_key.clone()) // Используем clone для api_key если он нужен дальше
            .header("X-BAPI-TIMESTAMP", timestamp.clone()) // Используем clone для timestamp если он нужен дальше
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .send()
            .await?;

        // Проверяем статус ответа
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(crate::error::Error(format!(
                "Ошибка API Bybit при получении позиций: статус {}, ответ: {}",
                status, error_text
            )));
        }

        let body: Value = response.json().await?;

        // Отладочный вывод полного ответа
        log::debug!("Bybit API get_positions response: {}", body.to_string());

        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка API Bybit");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit при получении позиций: код {}, сообщение: {}",
                    ret_code, ret_msg
                )));
            }
        }

        // Ожидаем, что API вернет список позиций в поле result.list
        Ok(body["result"]["list"].as_array().unwrap_or(&Vec::new()).clone())
    }

    pub async fn create_order_with_stop_loss_and_take_profit(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
        category: &str,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Result<String> {
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

        // Создаем объект с обязательными параметрами ордера
        let mut order_body = json!({
            "category": category,
            "symbol": symbol,
            "side": side,
            "orderType": "Limit",
            "qty": quantity.to_string(),
            "price": price.to_string(),
            "timeInForce": "GTC",
        });

        // Добавляем стоп-лосс, если он указан
        if let Some(sl_price) = stop_loss {
            order_body["stopLoss"] = json!(sl_price.to_string());
        }

        // Добавляем тейк-профит, если он указан
        if let Some(tp_price) = take_profit {
            order_body["takeProfit"] = json!(tp_price.to_string());
        }

        // Для POST запросов, используем тело JSON
        let body_str = order_body.to_string();

        // Формируем строку для подписи (для POST запросов): {timestamp}{api_key}{recv_window}{body}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, body_str);

        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);

        // Конструируем URL
        let url = format!("{}/v5/order/create", BASE_URL);

        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .post(&url)
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
                status, error_text
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
                    ret_code, ret_msg
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
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;

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

    pub async fn closed_pnl(
        &self,
        category: &str,
        symbol: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Vec<Value>> {
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

        // Формируем параметры запроса
        let mut params = vec![format!("category={}", category)];

        // Добавляем опциональные параметры, если они указаны
        if let Some(s) = symbol {
            params.push(format!("symbol={}", s));
        }

        if let Some(st) = start_time {
            params.push(format!("startTime={}", st));
        }

        if let Some(et) = end_time {
            params.push(format!("endTime={}", et));
        }

        if let Some(lim) = limit {
            params.push(format!("limit={}", lim));
        }

        if let Some(cur) = cursor {
            params.push(format!("cursor={}", cur));
        }

        let query_string = params.join("&");

        // Формируем строку для подписи: {timestamp}{api_key}{recv_window}{query_string}
        let signature_payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);

        // Создаем HMAC подпись
        let signature = Self::hmac_sha256(api_secret, signature_payload);

        // Конструируем URL с параметрами
        let url = format!("{}/v5/position/closed-pnl?{}", BASE_URL, query_string);

        let proxy = reqwest::Proxy::http(self._proxy.clone().unwrap())?;
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(15)).proxy(proxy).build()?;

        let response = client
            .get(&url)
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
                "Ошибка API Bybit при получении закрытых позиций: статус {}, ответ: {}",
                status, error_text
            )));
        }

        let body: Value = response.json().await?;

        // Отладочный вывод полного ответа
        log::debug!("Bybit API closed_pnl response: {}", body.to_string());

        // Проверяем ответ API на ошибки
        if let Some(ret_code) = body["retCode"].as_i64() {
            if ret_code != 0 {
                let ret_msg = body["retMsg"].as_str().unwrap_or("Неизвестная ошибка API Bybit");
                return Err(crate::error::Error(format!(
                    "Ошибка API Bybit при получении закрытых позиций: код {}, сообщение: {}",
                    ret_code, ret_msg
                )));
            }
        }

        // Возвращаем список закрытых позиций
        Ok(body["result"]["list"].as_array().unwrap_or(&Vec::new()).clone())
    }
}
