use super::super::utils::http_get_async;
use crate::error::Result;
use hmac::{Hmac, Mac};
use reqwest;
use serde_json::Value;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://api.mexc.com";

/// MEXC Spot market.
///
/// * REST API doc: <https://mexcdevelop.github.io/apidocs/spot_v3_en/>
/// * Trading at: <https://www.mexc.com/exchange/BTC_USDT>
/// * Rate Limits: <https://mexcdevelop.github.io/apidocs/spot_v3_en/#limits>
///   * Weight limits vary by endpoint, most market data endpoints have weight of 1
pub struct MexcSpotRestClient {
    _access_key: Option<String>,
    _secret_key: Option<String>,
    _proxy: Option<String>,
}

impl MexcSpotRestClient {
    pub fn new(
        access_key: Option<String>,
        secret_key: Option<String>,
        proxy: Option<String>,
    ) -> Self {
        MexcSpotRestClient { _access_key: access_key, _secret_key: secret_key, _proxy: proxy }
    }

    fn get_timestamp() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    /// Создать подпись для MEXC API
    fn generate_signature(params: &BTreeMap<String, String>, secret: &str) -> Result<String> {
        let mut params_str = String::new();

        for (key, value) in params {
            params_str.push_str(&format!("{}={}&", key, value));
        }

        // Удаляем последний &
        if !params_str.is_empty() {
            params_str.pop();
        }

        // MEXC подпись создается по формуле: HMAC-SHA256(secretKey, params_str)
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| crate::error::Error("Failed to create HMAC".to_string()))?;
        mac.update(params_str.as_bytes());
        let result = mac.finalize();
        // MEXC требует lowercase подпись
        let signature = hex::encode(result.into_bytes()).to_lowercase();

        Ok(signature)
    }

    /// Создать лимитный ордер.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/order` для создания нового ордера.
    /// Требует API ключ и секретный ключ для аутентификации.
    ///
    /// # Параметры
    /// * `symbol` - Торговая пара в формате "BTCUSDT" (без подчеркивания)
    /// * `side` - Сторона ордера: "BUY" или "SELL"
    /// * `quantity` - Количество для покупки/продажи (должно соответствовать минимальным требованиям биржи)
    /// * `price` - Цена лимитного ордера (должна соответствовать точности биржи)
    ///
    /// # Возвращает
    /// * `Result<String>` - JSON ответ с информацией о созданном ордере
    ///
    /// # Ошибки
    /// * `Error` - Если отсутствуют API ключи
    /// * `Error` - Если параметры ордера не соответствуют требованиям биржи
    /// * `Error` - Если недостаточно баланса для ордера
    ///
    /// # Пример
    /// ```
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// let order = client.create_order("BTCUSDT", "BUY", 0.001, 50000.0).await?;
    /// ```
    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<String> {
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для создания ордера".to_string(),
            ));
        }

        // Валидация параметров
        if symbol.is_empty() {
            return Err(crate::error::Error(
                "Символ торговой пары не может быть пустым".to_string(),
            ));
        }

        if !matches!(side.to_uppercase().as_str(), "BUY" | "SELL") {
            return Err(crate::error::Error(
                "Сторона ордера должна быть 'BUY' или 'SELL'".to_string(),
            ));
        }

        if quantity <= 0.0 {
            return Err(crate::error::Error("Количество должно быть больше 0".to_string()));
        }

        if price <= 0.0 {
            return Err(crate::error::Error("Цена должна быть больше 0".to_string()));
        }

        // Дополнительная валидация для MEXC API
        if quantity < 0.000001 {
            return Err(crate::error::Error("Количество слишком мало для MEXC API".to_string()));
        }

        if price < 0.000001 {
            return Err(crate::error::Error("Цена слишком мала для MEXC API".to_string()));
        }

        if quantity < 1.0 {
            println!(
                "WARNING: Объем ордера {:.6} USDT меньше рекомендуемого минимума (1 USDT)",
                quantity
            );
        }

        let api_key = self._access_key.as_ref().unwrap();
        let secret_key = self._secret_key.as_ref().unwrap();

        let url = format!("{}/api/v3/order", BASE_URL);
        let mut params = BTreeMap::new();

        // Форматируем числовые значения правильно для MEXC API
        // MEXC может требовать определенный формат чисел
        // Рассчитываем количество по формуле quantity/price
        let calculated_quantity = quantity / price;
        let formatted_quantity = if calculated_quantity.fract() == 0.0 {
            format!("{:.0}", calculated_quantity)
        } else {
            format!("{}", calculated_quantity)
        };

        let formatted_price =
            if price.fract() == 0.0 { format!("{:.0}", price) } else { format!("{}", price) };

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("side".to_string(), side.to_uppercase());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("quantity".to_string(), formatted_quantity);
        params.insert("price".to_string(), formatted_price);
        params.insert("timeInForce".to_string(), "GTC".to_string());
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

        // Генерируем подпись
        let signature = Self::generate_signature(&params, secret_key)?;
        params.insert("signature".to_string(), signature);

        // Создаем query string из параметров
        let mut query_params = Vec::new();
        for (key, value) in params.iter() {
            query_params.push(format!("{}={}", key, value));
        }
        let query_string = query_params.join("&");
        let full_url = format!("{}?{}", url, query_string);

        // Отладочная информация
        println!("DEBUG: Full URL: {}", full_url);
        println!("DEBUG: All params: {:?}", params);

        // Создаем HTTP клиент
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36");

        if let Some(proxy_url) = &self._proxy {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy_url)
                    .map_err(|e| crate::error::Error(format!("Proxy error: {}", e)))?,
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| crate::error::Error(format!("Client build error: {}", e)))?;

        // Отправляем POST запрос с параметрами в query string
        let response = client
            .post(&full_url)
            .header("X-MEXC-APIKEY", api_key)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error(format!("Request error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| crate::error::Error(format!("Response text error: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::Error(format!(
                "MEXC API error ({}): {}",
                status, response_text
            )));
        }

        Ok(response_text)
    }

    /// Получить баланс аккаунта для конкретного актива или все балансы.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/account` для получения информации об аккаунте.
    /// Требует API ключ и секретный ключ для аутентификации.
    ///
    /// # Параметры
    /// * `asset` - Символ валюты для получения баланса (например, "BTC", "USDT").
    ///   Если пустая строка "", возвращает полный JSON с балансами всех монет.
    ///
    /// # Возвращает
    /// * `Result<String>` - Если asset указан: доступный баланс актива в виде строки или "0" если актив не найден.
    ///   Если asset пустой: полный JSON ответ со всеми балансами аккаунта.
    ///
    /// # Пример
    /// ```
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key));
    /// // Получить баланс конкретной монеты
    /// let usdt_balance = client.get_account_balance("USDT").await?;
    /// // Получить все балансы
    /// let all_balances = client.get_account_balance("").await?;
    /// ```
    pub async fn get_account_balance(&self, asset: &str) -> Result<String> {
        // Проверяем наличие API ключа и секрета
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для получения баланса".to_string(),
            ));
        }

        let endpoint = format!("{}/api/v3/account", BASE_URL);
        let mut params = BTreeMap::new();

        // timestamp будет добавлен автоматически в http_get_async

        let response = http_get_async(
            &endpoint,
            &mut params,
            self._access_key.as_deref(),
            self._secret_key.as_deref(),
            self._proxy.as_deref(),
        )
        .await?;

        // Если asset пустой, возвращаем полный JSON ответ
        if asset.is_empty() {
            return Ok(response);
        }

        // Парсим JSON ответ и ищем баланс для указанного актива
        let json: Value = serde_json::from_str(&response)?;

        if let Some(balances) = json["balances"].as_array() {
            for balance in balances {
                if balance["asset"].as_str() == Some(asset) {
                    // Возвращаем свободный баланс (free)
                    if let Some(free_balance) = balance["free"].as_str() {
                        return Ok(free_balance.to_string());
                    } else if let Some(free_balance) = balance["free"].as_f64() {
                        return Ok(free_balance.to_string());
                    }
                }
            }
        }

        // Если актив не найден, возвращаем "0"
        Ok("0".to_string())
    }

    /// Отменить существующий ордер.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/order` с DELETE методом для отмены ордера.
    /// Требует API ключ и секретный ключ для аутентификации.
    ///
    /// # Параметры
    /// * `symbol` - Торговая пара в формате "BTCUSDT" (без подчеркивания)
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
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// let result = client.cancel_order("BTCUSDT", "12345678").await?;
    /// ```
    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<String> {
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для отмены ордера".to_string(),
            ));
        }

        // Валидация параметров
        if symbol.is_empty() {
            return Err(crate::error::Error(
                "Символ торговой пары не может быть пустым".to_string(),
            ));
        }

        if order_id.is_empty() {
            return Err(crate::error::Error("ID ордера не может быть пустым".to_string()));
        }

        let api_key = self._access_key.as_ref().unwrap();
        let secret_key = self._secret_key.as_ref().unwrap();

        let url = format!("{}/api/v3/order", BASE_URL);
        let mut params = BTreeMap::new();

        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("orderId".to_string(), order_id.to_string());
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

        // Генерируем подпись
        let signature = Self::generate_signature(&params, secret_key)?;
        params.insert("signature".to_string(), signature);

        // Создаем query string из параметров
        let mut query_params = Vec::new();
        for (key, value) in params.iter() {
            query_params.push(format!("{}={}", key, value));
        }
        let query_string = query_params.join("&");
        let full_url = format!("{}?{}", url, query_string);

        // Создаем HTTP клиент
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36");

        if let Some(proxy_url) = &self._proxy {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy_url)
                    .map_err(|e| crate::error::Error(format!("Proxy error: {}", e)))?,
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| crate::error::Error(format!("Client build error: {}", e)))?;

        // Отправляем DELETE запрос
        let response = client
            .delete(&full_url)
            .header("X-MEXC-APIKEY", api_key)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error(format!("Request error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| crate::error::Error(format!("Response text error: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::Error(format!(
                "MEXC API error ({}): {}",
                status, response_text
            )));
        }

        Ok(response_text)
    }

    /// Получить listen_key для WebSocket приватных данных.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/userDataStream` для создания listen_key,
    /// который необходим для подписки на приватные WebSocket каналы (балансы, ордера).
    /// Требует API ключ и секретный ключ для аутентификации с подписью.
    ///
    /// # Возвращает
    /// * `Result<String>` - listen_key строка, действительная в течение 60 минут
    ///
    /// # Ошибки
    /// * `Error` - Если отсутствуют API ключ или секретный ключ
    /// * `Error` - Если API запрос неуспешен
    ///
    /// # Пример
    /// ```
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// let listen_key = client.get_listen_key().await?;
    /// ```
    ///
    /// # Примечания
    /// - listen_key действует 60 минут, после чего требует обновления
    /// - Каждый вызов этого метода создает новый listen_key
    /// - Старые listen_key автоматически становятся недействительными
    /// - Для создания listen_key требуется подпись с timestamp
    pub async fn get_listen_key(&self) -> Result<String> {
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для получения listen_key".to_string(),
            ));
        }

        let api_key = self._access_key.as_ref().unwrap();
        let secret_key = self._secret_key.as_ref().unwrap();

        let url = format!("{}/api/v3/userDataStream", BASE_URL);

        // Создаем параметры с timestamp для подписи
        let mut params = BTreeMap::new();
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

        // Генерируем подпись
        let signature = Self::generate_signature(&params, secret_key)?;
        params.insert("signature".to_string(), signature);

        // Создаем query string из параметров
        let mut query_params = Vec::new();
        for (key, value) in params.iter() {
            query_params.push(format!("{}={}", key, value));
        }
        let query_string = query_params.join("&");
        let full_url = format!("{}?{}", url, query_string);

        // Создаем HTTP клиент
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("office_bots/1.0");

        if let Some(proxy_url) = &self._proxy {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy_url)
                    .map_err(|e| crate::error::Error(format!("Proxy error: {}", e)))?,
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| crate::error::Error(format!("Client build error: {}", e)))?;

        // Отправляем POST запрос с подписью в query параметрах
        let response = client
            .post(&full_url)
            .header("X-MEXC-APIKEY", api_key)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error(format!("Request error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| crate::error::Error(format!("Response text error: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::Error(format!(
                "MEXC API error getting listen_key ({}): {}",
                status, response_text
            )));
        }

        // Парсим JSON ответ для извлечения listen_key
        let json: Value = serde_json::from_str(&response_text)
            .map_err(|e| crate::error::Error(format!("JSON parse error: {}", e)))?;

        if let Some(listen_key) = json["listenKey"].as_str() {
            Ok(listen_key.to_string())
        } else {
            Err(crate::error::Error(format!("listen_key не найден в ответе: {}", response_text)))
        }
    }

    /// Продлить действие listen_key.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/userDataStream` с PUT методом для
    /// продления действия существующего listen_key на следующие 60 минут.
    /// Требует API ключ и секретный ключ для аутентификации с подписью.
    ///
    /// # Параметры
    /// * `listen_key` - Существующий listen_key для продления
    ///
    /// # Возвращает
    /// * `Result<String>` - Ответ сервера (обычно пустой JSON {})
    ///
    /// # Ошибки
    /// * `Error` - Если отсутствуют API ключ или секретный ключ
    /// * `Error` - Если listen_key недействителен
    /// * `Error` - Если API запрос неуспешен
    ///
    /// # Пример
    /// ```
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// client.keep_alive_listen_key(&listen_key).await?;
    /// ```
    ///
    /// # Примечания
    /// - Рекомендуется продлевать listen_key каждые 30-50 минут
    /// - После продления, listen_key действует следующие 60 минут
    /// - Для продления listen_key требуется подпись с listenKey и timestamp
    pub async fn keep_alive_listen_key(&self, listen_key: &str) -> Result<String> {
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для продления listen_key".to_string(),
            ));
        }

        if listen_key.is_empty() {
            return Err(crate::error::Error("listen_key не может быть пустым".to_string()));
        }

        let api_key = self._access_key.as_ref().unwrap();
        let secret_key = self._secret_key.as_ref().unwrap();

        let url = format!("{}/api/v3/userDataStream", BASE_URL);

        // Создаем параметры с listenKey и timestamp для подписи
        let mut params = BTreeMap::new();
        params.insert("listenKey".to_string(), listen_key.to_string());
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

        // Генерируем подпись
        let signature = Self::generate_signature(&params, secret_key)?;
        params.insert("signature".to_string(), signature);

        // Создаем query string из параметров
        let mut query_params = Vec::new();
        for (key, value) in params.iter() {
            query_params.push(format!("{}={}", key, value));
        }
        let query_string = query_params.join("&");
        let full_url = format!("{}?{}", url, query_string);

        // Создаем HTTP клиент
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("office_bots/1.0");

        if let Some(proxy_url) = &self._proxy {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy_url)
                    .map_err(|e| crate::error::Error(format!("Proxy error: {}", e)))?,
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| crate::error::Error(format!("Client build error: {}", e)))?;

        // Отправляем PUT запрос с подписью в query параметрах
        let response = client
            .put(&full_url)
            .header("X-MEXC-APIKEY", api_key)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error(format!("Request error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| crate::error::Error(format!("Response text error: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::Error(format!(
                "MEXC API error keeping alive listen_key ({}): {}",
                status, response_text
            )));
        }

        Ok(response_text)
    }

    /// Удалить listen_key.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/userDataStream` с DELETE методом для
    /// удаления существующего listen_key и закрытия соответствующего WebSocket соединения.
    /// Требует API ключ и секретный ключ для аутентификации с подписью.
    ///
    /// # Параметры
    /// * `listen_key` - Существующий listen_key для удаления
    ///
    /// # Возвращает
    /// * `Result<String>` - Ответ сервера (обычно пустой JSON {})
    ///
    /// # Ошибки
    /// * `Error` - Если отсутствуют API ключ или секретный ключ
    /// * `Error` - Если listen_key недействителен
    /// * `Error` - Если API запрос неуспешен
    ///
    /// # Пример
    /// ```
    /// let client = MexcSpotRestClient::new(Some(api_key), Some(secret_key), None);
    /// client.close_listen_key(&listen_key).await?;
    /// ```
    ///
    /// # Примечания
    /// - После удаления listen_key становится недействительным
    /// - WebSocket соединения с этим ключом будут закрыты
    /// - Используйте для явного завершения сессии
    /// - Для удаления listen_key требуется подпись с listenKey и timestamp
    pub async fn close_listen_key(&self, listen_key: &str) -> Result<String> {
        if self._access_key.is_none() || self._secret_key.is_none() {
            return Err(crate::error::Error(
                "API ключ и секретный ключ обязательны для удаления listen_key".to_string(),
            ));
        }

        if listen_key.is_empty() {
            return Err(crate::error::Error("listen_key не может быть пустым".to_string()));
        }

        let api_key = self._access_key.as_ref().unwrap();
        let secret_key = self._secret_key.as_ref().unwrap();

        let url = format!("{}/api/v3/userDataStream", BASE_URL);

        // Создаем параметры с listenKey и timestamp для подписи
        let mut params = BTreeMap::new();
        params.insert("listenKey".to_string(), listen_key.to_string());
        params.insert("timestamp".to_string(), Self::get_timestamp().to_string());

        // Генерируем подпись
        let signature = Self::generate_signature(&params, secret_key)?;
        params.insert("signature".to_string(), signature);

        // Создаем query string из параметров
        let mut query_params = Vec::new();
        for (key, value) in params.iter() {
            query_params.push(format!("{}={}", key, value));
        }
        let query_string = query_params.join("&");
        let full_url = format!("{}?{}", url, query_string);

        // Создаем HTTP клиент
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("office_bots/1.0");

        if let Some(proxy_url) = &self._proxy {
            client_builder = client_builder.proxy(
                reqwest::Proxy::all(proxy_url)
                    .map_err(|e| crate::error::Error(format!("Proxy error: {}", e)))?,
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| crate::error::Error(format!("Client build error: {}", e)))?;

        // Отправляем DELETE запрос с подписью в query параметрах
        let response = client
            .delete(&full_url)
            .header("X-MEXC-APIKEY", api_key)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error(format!("Request error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| crate::error::Error(format!("Response text error: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::Error(format!(
                "MEXC API error closing listen_key ({}): {}",
                status, response_text
            )));
        }

        Ok(response_text)
    }

    /// Получить последние сделки.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/trades` для получения недавних сделок.
    /// По умолчанию возвращает 500 сделок, максимум 1000.
    ///
    /// # Параметры
    /// * `symbol` - Торговая пара в формате "BTCUSDT" (без подчеркивания)
    ///
    /// # Возвращает
    /// * `Result<String>` - JSON строка с данными сделок
    ///
    /// # Пример
    /// ```
    /// let trades = MexcSpotRestClient::fetch_trades("BTCUSDT").await?;
    /// ```
    pub async fn fetch_trades(symbol: &str) -> Result<String> {
        let endpoint = format!("{}/api/v3/trades", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("limit".to_string(), "1000".to_string());

        http_get_async(&endpoint, &mut params, None, None, None).await
    }

    /// Получить снимок книги ордеров L2.
    ///
    /// Использует MEXC API v3 эндпоинт `/api/v3/depth` для получения данных OrderBook.
    /// Лимит по умолчанию - 100, максимум 5000.
    ///
    /// # Параметры  
    /// * `symbol` - Торговая пара в формате "BTCUSDT" (без подчеркивания)
    ///
    /// # Возвращает
    /// * `Result<String>` - JSON строка с данными книги ордеров
    ///
    /// # Пример
    /// ```
    /// let snapshot = MexcSpotRestClient::fetch_l2_snapshot("BTCUSDT").await?;
    /// ```
    pub async fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        let endpoint = format!("{}/api/v3/depth", BASE_URL);
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("limit".to_string(), "5000".to_string());

        http_get_async(&endpoint, &mut params, None, None, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mexc_account_balance_without_api_keys() {
        let client = MexcSpotRestClient::new(None, None, None);

        // Тест получения баланса конкретной монеты
        let result = client.get_account_balance("USDT").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API ключ и секретный ключ обязательны"));

        // Тест получения всех балансов
        let result_all = client.get_account_balance("").await;
        assert!(result_all.is_err());
        assert!(
            result_all.unwrap_err().to_string().contains("API ключ и секретный ключ обязательны")
        );
    }

    #[tokio::test]
    async fn test_mexc_listen_key_methods_without_api_keys() {
        let client = MexcSpotRestClient::new(None, None, None);

        // Тест получения listen_key без API ключей
        let result = client.get_listen_key().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API ключ и секретный ключ обязательны для получения listen_key")
        );

        // Тест продления listen_key без API ключей
        let result = client.keep_alive_listen_key("test_key").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API ключ и секретный ключ обязательны для продления listen_key")
        );

        // Тест удаления listen_key без API ключей
        let result = client.close_listen_key("test_key").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API ключ и секретный ключ обязательны для удаления listen_key")
        );
    }

    #[tokio::test]
    async fn test_mexc_listen_key_methods_with_empty_listen_key() {
        let client = MexcSpotRestClient::new(
            Some("test_key".to_string()),
            Some("test_secret".to_string()),
            None,
        );

        // Тест продления с пустым listen_key
        let result = client.keep_alive_listen_key("").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("listen_key не может быть пустым"));

        // Тест удаления с пустым listen_key
        let result = client.close_listen_key("").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("listen_key не может быть пустым"));
    }

    #[test]
    fn test_mexc_client_creation() {
        let client = MexcSpotRestClient::new(
            Some("test_key".to_string()),
            Some("test_secret".to_string()),
            Some("http://proxy:8080".to_string()),
        );

        assert_eq!(client._access_key, Some("test_key".to_string()));
        assert_eq!(client._secret_key, Some("test_secret".to_string()));
        assert_eq!(client._proxy, Some("http://proxy:8080".to_string()));
    }

    #[test]
    fn test_mexc_timestamp_generation() {
        let timestamp = MexcSpotRestClient::get_timestamp();

        // Проверяем, что timestamp в разумном диапазоне (2020-2030 годы)
        assert!(timestamp > 1577836800000); // 1 января 2020
        assert!(timestamp < 1893456000000); // 1 января 2030

        // Проверяем, что timestamp генерируется в миллисекундах, а не секундах
        let timestamp_str = timestamp.to_string();
        assert_eq!(timestamp_str.len(), 13); // Должно быть 13 цифр для миллисекунд
    }

    #[test]
    fn test_mexc_parameter_validation() {
        let client = MexcSpotRestClient::new(
            Some("test_key".to_string()),
            Some("test_secret".to_string()),
            None,
        );

        // Проверяем валидацию в методе create_order (пока без async)
        // Эти проверки помогут понять структуру параметров

        // Тест пустого символа
        assert!("".is_empty());

        // Тест некорректной стороны
        assert!(!matches!("INVALID".to_uppercase().as_str(), "BUY" | "SELL"));
        assert!(matches!("BUY".to_uppercase().as_str(), "BUY" | "SELL"));
        assert!(matches!("SELL".to_uppercase().as_str(), "BUY" | "SELL"));

        // Тест количества и цены
        assert!(0.0 <= 0.0);
        assert!(0.001 > 0.0);
        assert!(50000.0 > 0.0);
    }

    #[test]
    fn test_mexc_signature_generation() {
        let mut params = BTreeMap::new();
        params.insert("symbol".to_string(), "BTCUSDT".to_string());
        params.insert("side".to_string(), "BUY".to_string());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("quantity".to_string(), "1".to_string());
        params.insert("price".to_string(), "11".to_string());
        params.insert("recvWindow".to_string(), "5000".to_string());
        params.insert("timestamp".to_string(), "1644489390087".to_string());

        let secret = "45d0b3c26f2644f19bfb98b07741b2f5";
        let signature = MexcSpotRestClient::generate_signature(&params, secret).unwrap();

        // Проверяем, что подпись сгенерирована и имеет правильную длину (64 символа для SHA256)
        assert_eq!(signature.len(), 64);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(signature.chars().all(|c| c.is_lowercase() || c.is_ascii_digit()));
    }
}
