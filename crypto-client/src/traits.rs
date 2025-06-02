use crate::exchange_type::ExchangeType;
use async_trait::async_trait;

/// Базовый трейт для всех клиентов бирж
#[async_trait]
pub trait ExchangeClient: Send + Sync {
    /// Получить тип биржи
    fn exchange_type(&self) -> ExchangeType;

    /// Получить снимок orderbook уровня 2
    async fn fetch_l2_snapshot(&self, symbol: &str) -> Result<String, String>;

    /// Получить баланс аккаунта
    async fn get_balance(&self, asset: &str) -> Result<String, String>;

    /// Создать лимитный ордер
    async fn create_limit_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<String, String>;

    /// Отменить ордер
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<String, String>;

    /// Получить listen_key для WebSocket приватных данных
    ///
    /// # Возвращает
    /// * `Result<String, String>` - listen_key для WebSocket подписок или ошибка
    ///
    /// # Примечания
    /// - Не все биржи поддерживают этот метод
    /// - listen_key обычно действует ограниченное время (например, 60 минут для MEXC)
    async fn get_listen_key(&self) -> Result<String, String> {
        Err("get_listen_key не поддерживается для этой биржи".to_string())
    }
}

/// Трейт для WebSocket клиентов
#[async_trait]
pub trait WebSocketClient: Send + Sync {
    /// Тип сообщения, которое получает клиент
    type Message;

    /// Подключиться к WebSocket
    async fn connect(&mut self) -> Result<(), String>;

    /// Отключиться от WebSocket
    async fn disconnect(&mut self) -> Result<(), String>;

    /// Подписаться на orderbook
    async fn subscribe_orderbook(&mut self, symbol: &str) -> Result<(), String>;

    /// Подписаться на сделки
    async fn subscribe_trades(&mut self, symbol: &str) -> Result<(), String>;

    /// Подписаться на тикеры
    async fn subscribe_ticker(&mut self, symbol: &str) -> Result<(), String>;

    /// Подписаться на баланс аккаунта
    ///
    /// # Параметры
    /// * `listen_key` - Опциональный ключ для приватных каналов (например, для MEXC)
    ///   Если None, клиент попытается подписаться без аутентификации
    async fn subscribe_account_balance(&mut self, listen_key: Option<&str>) -> Result<(), String>;

    /// Подписаться на приватные сделки аккаунта
    ///
    /// # Параметры
    /// * `listen_key` - Опциональный ключ для приватных каналов (например, для MEXC)
    ///   Если None, клиент попытается подписаться без аутентификации
    async fn subscribe_private_deals(&mut self, listen_key: Option<&str>) -> Result<(), String>;

    /// Получить следующее сообщение
    async fn next_message(&mut self) -> Result<Option<Self::Message>, String>;

    /// Проверить, подключён ли клиент
    fn is_connected(&self) -> bool;
}

/// Трейт для управления подписками
pub trait SubscriptionManager {
    /// Добавить подписку
    fn add_subscription(&mut self, channel: String, symbol: String);

    /// Удалить подписку
    fn remove_subscription(&mut self, channel: String, symbol: String);

    /// Получить все активные подписки
    fn get_subscriptions(&self) -> Vec<(String, String)>;

    /// Очистить все подписки
    fn clear_subscriptions(&mut self);
}
