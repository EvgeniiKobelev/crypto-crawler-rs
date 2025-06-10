/// Конфигурация для клиента биржи
#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub password: Option<String>,
    pub proxy: Option<String>,
    pub testnet: bool,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self { api_key: None, secret_key: None, password: None, proxy: None, testnet: false }
    }
}

impl ExchangeConfig {
    /// Создать новую конфигурацию с API ключами
    pub fn new(api_key: Option<String>, secret_key: Option<String>) -> Self {
        Self { api_key, secret_key, password: None, proxy: None, testnet: false }
    }

    /// Создать конфигурацию с API ключами и password (для OKX, KuCoin)
    pub fn with_password(
        api_key: Option<String>,
        secret_key: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self { api_key, secret_key, password, proxy: None, testnet: false }
    }

    /// Установить прокси
    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }

    /// Установить режим тестовой сети
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Проверить, установлены ли необходимые ключи
    pub fn has_auth_keys(&self) -> bool {
        self.api_key.is_some() && self.secret_key.is_some()
    }
}
