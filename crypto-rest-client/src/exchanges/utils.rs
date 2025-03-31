use reqwest::{blocking::Response, header};
use crate::error::{Error, Result};
use std::collections::BTreeMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

#[allow(dead_code)]
fn generate_signature(params: &BTreeMap<String, String>, secret: &str) -> Result<String> {
    let mut params_str = String::new();
    
    for (key, value) in params {
        params_str.push_str(&format!("{}={}&", key, value));
    }
    
    // Удаляем последний &
    if !params_str.is_empty() {
        params_str.pop();
    }
    
    // Подпись является HMAC-SHA256 хешем
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| Error("Failed to create HMAC".to_string()))?;
    mac.update(params_str.as_bytes());
    let result = mac.finalize();
    let signature = hex::encode(result.into_bytes());
    
    Ok(signature)
}

// Returns the raw response directly.
pub(super) fn http_get_raw(url: &str, params: &BTreeMap<String, String>) -> Result<Response> {
    let mut full_url = url.to_string();
    let mut first = true;
    for (k, v) in params.iter() {
        if first {
            full_url.push_str(format!("?{k}={v}").as_str());
            first = false;
        } else {
            full_url.push_str(format!("&{k}={v}").as_str());
        }
    }
    // println!("{}", full_url);

    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));

    let client = reqwest::blocking::Client::builder()
         .default_headers(headers)
         .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36")
         .gzip(true)
         .build()?;
    let response = client.get(full_url.as_str()).send()?;
    Ok(response)
}

// Returns the text in response.
pub(super) fn http_get(url: &str, params: &BTreeMap<String, String>) -> Result<String> {
    match http_get_raw(url, params) {
        Ok(response) => match response.error_for_status() {
            Ok(resp) => Ok(resp.text()?),
            Err(error) => Err(Error::from(error)),
        },
        Err(err) => Err(err),
    }
}

// Асинхронные методы
pub(super) async fn http_get_raw_async(
    url: &str, 
    params: &mut BTreeMap<String, String>,
    api_key: Option<&str>,
    api_secret: Option<&str>,
    proxy: Option<&str>,
) -> Result<reqwest::Response> {
    // Добавляем timestamp для Binance API
    if api_key.is_some() && api_secret.is_some() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        params.insert("timestamp".to_string(), timestamp.clone());
        
        // Генерируем подпись
        let signature = generate_signature(params, api_secret.unwrap())?;
        params.insert("signature".to_string(), signature);
    }

    let mut full_url = url.to_string();
    let mut first = true;
    for (k, v) in params.iter() {
        if first {
            full_url.push_str(format!("?{k}={v}").as_str());
            first = false;
        } else {
            full_url.push_str(format!("&{k}={v}").as_str());
        }
    }

    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
    
    if let Some(key) = api_key {
        headers.insert("X-MBX-APIKEY", header::HeaderValue::from_str(key).map_err(|e| Error::from(e))?);
    }

    let mut client_builder = reqwest::Client::builder()
        .default_headers(headers)
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36")
        .gzip(true);

    if let Some(proxy_url) = proxy {
        client_builder = client_builder.proxy(reqwest::Proxy::all(proxy_url).map_err(|e| Error::from(e))?);
    }

    let client = client_builder.build().map_err(|e| Error::from(e))?;
    let response = client.get(full_url.as_str()).send().await.map_err(|e| Error::from(e))?;
    Ok(response)
}

pub(super) async fn http_get_async(
    url: &str, 
    params: &mut BTreeMap<String, String>,
    api_key: Option<&str>,
    api_secret: Option<&str>,
    proxy: Option<&str>,
) -> Result<String> {
    match http_get_raw_async(url, params, api_key, api_secret, proxy).await {
        Ok(response) => match response.error_for_status() {
            Ok(resp) => Ok(resp.text().await?),
            Err(error) => {
                // Создаем информативную ошибку
                Err(crate::error::Error(format!("API Error: {} - Проверьте параметры запроса.", error)))
            },
        },
        Err(err) => Err(err),
    }
}

pub(super) async fn http_post_async(
    url: &str, 
    params: &mut BTreeMap<String, String>,
    api_key: Option<&str>,
    api_secret: Option<&str>,
    proxy: Option<&str>,
) -> Result<String> {
    // Шаг 1: Добавляем timestamp, если используется авторизация
    if api_key.is_some() && api_secret.is_some() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        params.insert("timestamp".to_string(), timestamp.clone());
        
        // Шаг 2: Генерируем подпись на основе всех параметров
        let mut params_str = String::new();
        for (key, value) in params.iter() {
            params_str.push_str(&format!("{}={}&", key, value));
        }
        if !params_str.is_empty() {
            params_str.pop(); // Удаляем последний &
        }
        
        // Создаем HMAC-SHA256 подпись
        let mut mac = HmacSha256::new_from_slice(api_secret.unwrap().as_bytes())
            .map_err(|_| Error("Failed to create HMAC".to_string()))?;
        mac.update(params_str.as_bytes());
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());
        
        // Шаг 3: Добавляем подпись к параметрам
        params.insert("signature".to_string(), signature);
    }
    
    // Шаг 4: Формируем строку запроса для URL
    let mut query_string = String::new();
    for (key, value) in params.iter() {
        if !query_string.is_empty() {
            query_string.push('&');
        }
        query_string.push_str(&format!("{}={}", key, value));
    }
    
    // Шаг 5: Создаем полный URL с параметрами
    let full_url = format!("{}?{}", url, query_string);
    
    // Шаг 6: Подготавливаем заголовки
    let mut headers = header::HeaderMap::new();
    headers.insert(header::ACCEPT, header::HeaderValue::from_static("application/json"));
    
    if let Some(key) = api_key {
        headers.insert("X-MBX-APIKEY", header::HeaderValue::from_str(key).map_err(|e| Error::from(e))?);
    }
    
    // Шаг 7: Создаем HTTP клиент
    let mut client_builder = reqwest::Client::builder()
        .default_headers(headers)
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36");
    
    if let Some(proxy_url) = proxy {
        client_builder = client_builder.proxy(reqwest::Proxy::all(proxy_url).map_err(|e| Error::from(e))?);
    }
    
    let client = client_builder.build().map_err(|e| Error::from(e))?;
    
    // Шаг 8: Отправляем POST запрос без тела, все параметры в URL
    let response = client.post(&full_url)
        .send()
        .await
        .map_err(|e| Error::from(e))?;
    
    // Шаг 9: Обрабатываем ответ
    let status = response.status();
    
    match response.error_for_status() {
        Ok(resp) => {
            let text = resp.text().await?;
            Ok(text)
        },
        Err(error) => {
            // Пытаемся получить тело ответа с ошибкой
            if let Some(status_code) = error.status() {
                // Для ошибки 400 выводим дополнительную информацию
                if status_code == reqwest::StatusCode::BAD_REQUEST {
                    println!("Ошибка 400 Bad Request: проверьте точность количества и цены");
                }
            }
            
            Err(crate::error::Error(format!("API Error: {} - Проверьте параметры запроса.", error)))
        },
    }
}

macro_rules! gen_api {
    ( $path:expr$(, $param_name:ident )* ) => {
        {
            #[allow(unused_mut)]
            let mut params = BTreeMap::new();
            $(
                if let Some(param_name) = $param_name {
                    params.insert(stringify!($param_name).to_string(), param_name.to_string());
                }
            )*
            let url = if $path.starts_with("http") { $path.to_string() } else { format!("{}{}",BASE_URL, $path) };
            http_get(&url, &params)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    // System proxies are enabled by default, see <https://docs.rs/reqwest/latest/reqwest/#proxies>
    #[test]
    #[ignore]
    fn use_system_socks_proxy() {
        std::env::set_var("https_proxy", "socks5://127.0.0.1:9050");
        let text =
            super::http_get("https://check.torproject.org/api/ip", &BTreeMap::new()).unwrap();
        let obj = serde_json::from_str::<BTreeMap<String, Value>>(&text).unwrap();
        assert!(obj.get("IsTor").unwrap().as_bool().unwrap());
    }

    #[test]
    #[ignore]
    fn use_system_https_proxy() {
        std::env::set_var("https_proxy", "http://127.0.0.1:8118");
        let text =
            super::http_get("https://check.torproject.org/api/ip", &BTreeMap::new()).unwrap();
        let obj = serde_json::from_str::<BTreeMap<String, Value>>(&text).unwrap();
        assert!(obj.get("IsTor").unwrap().as_bool().unwrap());
    }
}
