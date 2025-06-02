//! Унифицированный клиент для работы с криптовалютными биржами
//!
//! Этот крейт предоставляет единый интерфейс для работы с различными
//! криптовалютными биржами через REST API и WebSocket соединения.

pub use crypto_market_type::MarketType;
use serde::{Deserialize, Serialize};

// Модули
pub mod config;
pub mod exchange_type;
pub mod rest_client;
pub mod traits;
pub mod ws_client;

// Экспорт основных типов и структур
pub use config::ExchangeConfig;
pub use exchange_type::ExchangeType;
pub use rest_client::{CryptoRestClient, ExchangeClientFactory, RestClientWrapper};
pub use traits::{ExchangeClient, SubscriptionManager, WebSocketClient};
pub use ws_client::{
    ChannelType, ConnectionState, CryptoWsClient, SubscriptionConfig, WsClientFactory,
    WsClientWrapper, WsMessage,
};

/// Результат операции с биржей
pub type ExchangeResult<T> = Result<T, ExchangeError>;

/// Ошибки при работе с биржами
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExchangeError {
    /// Ошибка конфигурации
    ConfigError(String),
    /// Ошибка сети
    NetworkError(String),
    /// Ошибка аутентификации
    AuthError(String),
    /// Ошибка API биржи
    ApiError(String),
    /// Ошибка парсинга данных
    ParseError(String),
    /// Биржа не поддерживается
    UnsupportedExchange(String),
    /// WebSocket ошибка
    WebSocketError(String),
    /// Общая ошибка
    GeneralError(String),
}

impl std::fmt::Display for ExchangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExchangeError::ConfigError(msg) => write!(f, "Ошибка конфигурации: {}", msg),
            ExchangeError::NetworkError(msg) => write!(f, "Ошибка сети: {}", msg),
            ExchangeError::AuthError(msg) => write!(f, "Ошибка аутентификации: {}", msg),
            ExchangeError::ApiError(msg) => write!(f, "Ошибка API: {}", msg),
            ExchangeError::ParseError(msg) => write!(f, "Ошибка парсинга: {}", msg),
            ExchangeError::UnsupportedExchange(msg) => {
                write!(f, "Биржа не поддерживается: {}", msg)
            }
            ExchangeError::WebSocketError(msg) => write!(f, "WebSocket ошибка: {}", msg),
            ExchangeError::GeneralError(msg) => write!(f, "Общая ошибка: {}", msg),
        }
    }
}

impl std::error::Error for ExchangeError {}

impl From<String> for ExchangeError {
    fn from(msg: String) -> Self {
        ExchangeError::GeneralError(msg)
    }
}

/// Конфигурация клиента для нескольких бирж
#[derive(Debug, Clone)]
pub struct MultiExchangeConfig {
    pub exchanges: Vec<(ExchangeType, ExchangeConfig)>,
    pub default_timeout: Option<u64>,
    pub retry_attempts: u32,
}

impl Default for MultiExchangeConfig {
    fn default() -> Self {
        Self { exchanges: Vec::new(), default_timeout: Some(30), retry_attempts: 3 }
    }
}

impl MultiExchangeConfig {
    /// Создать новую конфигурацию
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавить биржу в конфигурацию
    pub fn add_exchange(mut self, exchange_type: ExchangeType, config: ExchangeConfig) -> Self {
        self.exchanges.push((exchange_type, config));
        self
    }

    /// Установить таймаут по умолчанию
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.default_timeout = Some(timeout);
        self
    }

    /// Установить количество попыток переподключения
    pub fn with_retry_attempts(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }
}

/// Унифицированный клиент для работы с REST API и WebSocket
pub struct CryptoClient {
    pub rest_client: CryptoRestClient,
    pub ws_client: CryptoWsClient,
}

impl CryptoClient {
    /// Создать новый клиент
    pub fn new() -> Self {
        Self { rest_client: CryptoRestClient::new(), ws_client: CryptoWsClient::new() }
    }

    /// Создать клиент из конфигурации
    pub async fn from_config(config: MultiExchangeConfig) -> ExchangeResult<Self> {
        let mut client = Self::new();

        for (exchange_type, exchange_config) in config.exchanges {
            // Добавляем REST клиент
            client
                .rest_client
                .add_exchange(exchange_type.clone(), exchange_config.clone())
                .map_err(ExchangeError::ConfigError)?;

            // Добавляем WebSocket клиент, если поддерживается
            if exchange_type.supports_websocket() {
                client
                    .ws_client
                    .add_exchange(exchange_type, exchange_config)
                    .await
                    .map_err(ExchangeError::ConfigError)?;
            }
        }

        Ok(client)
    }

    /// Получить список доступных бирж для REST API
    pub fn get_rest_exchanges(&self) -> Vec<ExchangeType> {
        self.rest_client.get_available_exchanges()
    }

    /// Получить список доступных бирж для WebSocket
    pub fn get_ws_exchanges(&self) -> Vec<ExchangeType> {
        ExchangeType::all().into_iter().filter(|e| e.supports_websocket()).collect()
    }

    /// Проверить доступность биржи для REST API
    pub fn is_rest_available(&self, exchange_type: &ExchangeType) -> bool {
        self.rest_client.is_exchange_available(exchange_type)
    }

    /// Проверить доступность биржи для WebSocket
    pub fn is_ws_available(&self, exchange_type: &ExchangeType) -> bool {
        exchange_type.supports_websocket()
    }
}

impl Default for CryptoClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_type_as_str() {
        assert_eq!(ExchangeType::BinanceSpot.as_str(), "binance_spot");
        assert_eq!(ExchangeType::KucoinSpot.as_str(), "kucoin_spot");
    }

    #[test]
    fn test_exchange_type_supports_websocket() {
        assert!(ExchangeType::BinanceSpot.supports_websocket());
        assert!(ExchangeType::KucoinSpot.supports_websocket());
        assert!(!ExchangeType::BitstampSpot.supports_websocket());
    }

    #[test]
    fn test_config_creation() {
        let config =
            ExchangeConfig::new(Some("api_key".to_string()), Some("secret_key".to_string()));
        assert!(config.has_auth_keys());
    }

    #[test]
    fn test_multi_exchange_config() {
        let config = MultiExchangeConfig::new()
            .add_exchange(
                ExchangeType::BinanceSpot,
                ExchangeConfig::new(Some("key1".to_string()), Some("secret1".to_string())),
            )
            .add_exchange(
                ExchangeType::KucoinSpot,
                ExchangeConfig::new(Some("key2".to_string()), Some("secret2".to_string())),
            )
            .with_timeout(60)
            .with_retry_attempts(5);

        assert_eq!(config.exchanges.len(), 2);
        assert_eq!(config.default_timeout, Some(60));
        assert_eq!(config.retry_attempts, 5);
    }

    #[test]
    fn test_crypto_client_creation() {
        let client = CryptoClient::new();
        assert_eq!(client.rest_client.exchange_count(), 0);
        assert_eq!(client.ws_client.client_count(), 0);
    }
}
