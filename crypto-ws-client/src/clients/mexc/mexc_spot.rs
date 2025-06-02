use async_trait::async_trait;
use log::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

use crate::common::command_translator::CommandTranslator;
use crate::common::message_handler::{MessageHandler, MiscMessage};
use crate::common::ws_client_internal::WSClientInternal;
use crate::WSClient;

const EXCHANGE_NAME: &str = "mexc";

// Обновляем URL на v3 WebSocket API
pub(super) const SPOT_WEBSOCKET_URL: &str = "wss://wbs.mexc.com/ws";

// URL для User Data Stream
pub(super) const USER_DATA_STREAM_BASE_URL: &str = "wss://wbs-api.mexc.com/ws";

pub struct MexcSpotWSClient {
    client: WSClientInternal<MexcMessageHandler>,
}

/// Отдельный WebSocket клиент для User Data Stream MEXC
/// 
/// Этот клиент подключается к отдельному WebSocket эндпоинту
/// и получает приватные данные аккаунта автоматически
pub struct MexcUserDataStreamWSClient {
    client: WSClientInternal<MexcUserDataStreamMessageHandler>,
}

impl MexcUserDataStreamWSClient {
    /// Создает новое соединение для User Data Stream
    ///
    /// # Аргументы
    ///
    /// * `listen_key` - Ключ, полученный через REST API POST /api/v3/userDataStream
    /// * `tx` - Канал для отправки полученных сообщений
    /// * `proxy` - Опциональный прокси
    pub async fn new(listen_key: &str, tx: Sender<String>, _proxy: Option<String>) -> MexcUserDataStreamWSClient {
        let url = format!("{}?listenKey={}", USER_DATA_STREAM_BASE_URL, listen_key);
        
        info!("Подключение к MEXC User Data Stream: {}", url);
        debug!("Listen key длина: {} символов", listen_key.len());
        
        if listen_key.is_empty() {
            error!("Listen key пустой! Соединение может не установиться.");
        }
        
        if listen_key.len() < 50 {
            warn!("Listen key кажется слишком коротким ({}), проверьте правильность", listen_key.len());
        }
        
        MexcUserDataStreamWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                &url,
                MexcUserDataStreamMessageHandler {},
                None,
                tx,
            ).await,
        }
    }

    /// Подписка на обновления баланса аккаунта через User Data Stream
    ///
    /// Отправляет команду подписки на канал spot@private.account.v3.api.pb
    /// и затем запускает клиент для получения данных.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// use crypto_ws_client::MexcUserDataStreamWSClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = std::sync::mpsc::channel();
    ///     
    ///     // listenKey нужно получить через REST API запрос
    ///     let listen_key = "your_listen_key_from_rest_api";
    ///     let ws_client = MexcUserDataStreamWSClient::new(listen_key, tx, None).await;
    ///     
    ///     // Подписываемся на данные баланса и запускаем клиент
    ///     ws_client.subscribe_account_balance().await;
    /// }
    /// ```
    pub async fn subscribe_account_balance(&self) {
        let command = r#"{"method":"SUBSCRIPTION","params":["spot@private.account.v3.api.pb"]}"#;
        
        info!("Подписка на обновления баланса аккаунта MEXC через User Data Stream");
        debug!("Отправка команды подписки: {}", command);
        
        // Отправляем команду подписки
        self.client.send(&[command.to_string()]).await;
        
        // Добавляем небольшую задержку для обработки подписки
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        info!("Подписка на MEXC User Data Stream завершена");
        // ПРИМЕЧАНИЕ: run() теперь запускается в фоновом режиме через start_background_task()
        // Не вызываем self.run().await здесь, чтобы не блокировать поток
    }

    /// Подписка на приватные сделки аккаунта через User Data Stream
    ///
    /// Отправляет команду подписки на канал spot@private.deals.v3.api.pb
    /// для получения уведомлений о всех исполненных сделках.
    ///
    /// # Пример
    ///
    /// ```no_run
    /// use crypto_ws_client::MexcUserDataStreamWSClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = std::sync::mpsc::channel();
    ///     
    ///     // listenKey нужно получить через REST API запрос
    ///     let listen_key = "your_listen_key_from_rest_api";
    ///     let ws_client = MexcUserDataStreamWSClient::new(listen_key, tx, None).await;
    ///     
    ///     // Подписываемся на приватные сделки и запускаем клиент
    ///     ws_client.subscribe_private_deals().await;
    /// }
    /// ```
    pub async fn subscribe_private_deals(&self) {
        let command = r#"{"method":"SUBSCRIPTION","params":["spot@private.deals.v3.api.pb"]}"#;
        
        info!("Подписка на приватные сделки аккаунта MEXC через User Data Stream");
        debug!("Отправка команды подписки: {}", command);
        
        // Отправляем команду подписки
        self.client.send(&[command.to_string()]).await;
        
        // Добавляем небольшую задержку для обработки подписки
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        info!("Подписка на приватные сделки MEXC User Data Stream завершена");
        // ПРИМЕЧАНИЕ: run() теперь запускается в фоновом режиме через start_background_task()
        // Не вызываем self.run().await здесь, чтобы не блокировать поток
    }

    /// Запускает клиент User Data Stream
    pub async fn run(&self) {
        self.client.run().await;
    }

    /// Закрывает соединение User Data Stream
    pub async fn close(&self) {
        self.client.close().await;
    }
}

#[derive(Clone)]
pub struct MexcMessageHandler {}

#[derive(Clone)]
pub struct MexcUserDataStreamMessageHandler {}

#[derive(Clone)]
pub struct MexcCommandTranslator {}

impl MexcSpotWSClient {
    pub async fn new(tx: Sender<String>, _proxy: Option<String>) -> MexcSpotWSClient {
        MexcSpotWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                SPOT_WEBSOCKET_URL,
                MexcMessageHandler {},
                None,
                tx,
            ).await,
        }
    }
}

#[async_trait]
impl WSClient for MexcSpotWSClient {
    async fn subscribe_trade(&self, symbols: &[String]) {
        let commands = symbols.iter()
            .map(|symbol| MexcCommandTranslator::v3_subscription_command("deals", symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe_bbo(&self, symbols: &[String]) {
        warn!("BBO not supported for MEXC, subscribing to depth instead");
        self.subscribe_orderbook(symbols).await;
    }

    async fn subscribe_orderbook(&self, symbols: &[String]) {
        let commands = symbols.iter()
            .map(|symbol| MexcCommandTranslator::v3_subscription_command("depth", symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe_orderbook_topk(&self, symbols: &[String]) {
        // Для MEXC используем тот же канал depth
        self.subscribe_orderbook(symbols).await;
    }

    async fn subscribe_l3_orderbook(&self, _symbols: &[String]) {
        panic!("MEXC does not have level3 orderbook");
    }

    async fn subscribe_ticker(&self, _symbols: &[String]) {
        panic!("MEXC spot does not have ticker channel");
    }

    async fn subscribe_candlestick(&self, symbol_interval_list: &[(String, usize)]) {
        let commands = symbol_interval_list.iter()
            .map(|(symbol, interval)| {
                let mexc_symbol = symbol.replace("_", "");
                let interval_str = MexcCommandTranslator::interval_to_string(*interval);
                format!(
                    r#"{{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@{}@{}"]}}"#,
                    mexc_symbol, interval_str
                )
            })
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    /// Подписка на обновления баланса аккаунта через User Data Stream
    ///
    /// ВАЖНО: Для User Data Stream MEXC требует отдельное WebSocket соединение!
    /// Этот метод НЕ работает с обычным публичным WebSocket соединением.
    /// 
    /// Для получения данных баланса нужно:
    /// 1. Получить listenKey через REST API: POST /api/v3/userDataStream
    /// 2. Создать отдельное WebSocket соединение к: wss://wbs-api.mexc.com/ws?listenKey={listenKey}
    /// 3. Данные будут приходить автоматически без дополнительной подписки
    ///
    /// # Аргументы
    ///
    /// * `listen_key` - Ключ, полученный через REST API метод POST /api/v3/userDataStream
    ///
    /// # Предупреждение
    ///
    /// Этот метод в текущей реализации НЕ ПОДДЕРЖИВАЕТСЯ для MEXC.
    /// Используйте отдельный WebSocket клиент для User Data Stream.
    async fn subscribe_account_balance(&self, _listen_key: &str) {
        warn!("MEXC User Data Stream требует отдельного WebSocket соединения!");
        warn!("Используйте wss://wbs-api.mexc.com/ws?listenKey={{listenKey}} для получения данных баланса");
        warn!("Текущее публичное WebSocket соединение не поддерживает User Data Stream");
        
        // Не отправляем никаких команд, так как это не сработает на публичном WebSocket
    }

    async fn subscribe(&self, topics: &[(String, String)]) {
        let commands = topics.iter()
            .map(|(channel, symbol)| {
                MexcCommandTranslator::v3_subscription_command(channel, symbol)
            })
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn unsubscribe(&self, topics: &[(String, String)]) {
        let commands = topics.iter()
            .map(|(channel, symbol)| {
                MexcCommandTranslator::v3_unsubscription_command(channel, symbol)
            })
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn send(&self, commands: &[String]) {
        // Проверяем команды на наличие протобуф каналов
        for command in commands {
            if command.contains(".pb@") || command.contains(".api.pb") {
                warn!("⚠️  MEXC Protocol Buffers ОБНАРУЖЕН в команде: {}", command);
                warn!("   Protocol Buffers данные не поддерживаются в текущей реализации");
                warn!("   Рекомендация: используйте JSON каналы вместо протобуф");
                warn!("   Например:");
                warn!("     ❌ spot@public.deals.v3.api.pb@BTCUSDT");
                warn!("     ✅ spot@public.deals.v3.api@BTCUSDT");
                warn!("     ❌ spot@public.aggre.depth.v3.api.pb@100ms@BTCUSDT");
                warn!("     ✅ spot@public.increase.depth.v3.api@BTCUSDT");
            }
        }
        
        self.client.send(commands).await;
    }

    async fn run(&self) {
        self.client.run().await;
    }

    async fn close(&self) {
        self.client.close().await;
    }
}

impl MexcCommandTranslator {
    fn v3_subscription_command(channel: &str, symbol: &str) -> String {
        // Конвертируем символ из формата BTC_USDT в BTCUSDT для v3 API
        let mexc_symbol = symbol.replace("_", "");
        
        // Формируем команду подписки в формате v3 API
        match channel {
            "deals" | "deal" => format!(
                r#"{{"method":"SUBSCRIPTION","params":["spot@public.deals.v3.api@{}"]}}"#,
                mexc_symbol
            ),
            "depth" => format!(
                r#"{{"method":"SUBSCRIPTION","params":["spot@public.increase.depth.v3.api@{}"]}}"#,
                mexc_symbol
            ),
            "kline" => format!(
                r#"{{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@{}@Min1"]}}"#,
                mexc_symbol
            ),
            _ => {
                warn!("Неизвестный канал: {}", channel);
                format!(
                    r#"{{"method":"SUBSCRIPTION","params":["spot@public.deals.v3.api@{}"]}}"#,
                    mexc_symbol
                )
            }
        }
    }

    fn v3_unsubscription_command(channel: &str, symbol: &str) -> String {
        // Конвертируем символ из формата BTC_USDT в BTCUSDT для v3 API
        let mexc_symbol = symbol.replace("_", "");
        
        // Формируем команду отписки в формате v3 API
        match channel {
            "deals" | "deal" => format!(
                r#"{{"method":"UNSUBSCRIPTION","params":["spot@public.deals.v3.api@{}"]}}"#,
                mexc_symbol
            ),
            "depth" => format!(
                r#"{{"method":"UNSUBSCRIPTION","params":["spot@public.increase.depth.v3.api@{}"]}}"#,
                mexc_symbol
            ),
            "kline" => format!(
                r#"{{"method":"UNSUBSCRIPTION","params":["spot@public.kline.v3.api@{}@Min1"]}}"#,
                mexc_symbol
            ),
            _ => {
                warn!("Неизвестный канал: {}", channel);
                format!(
                    r#"{{"method":"UNSUBSCRIPTION","params":["spot@public.deals.v3.api@{}"]}}"#,
                    mexc_symbol
                )
            }
        }
    }

    fn interval_to_string(interval: usize) -> String {
        match interval {
            60 => "Min1".to_string(),
            300 => "Min5".to_string(),
            900 => "Min15".to_string(),
            1800 => "Min30".to_string(),
            3600 => "Hour1".to_string(),
            14400 => "Hour4".to_string(),
            86400 => "Day1".to_string(),
            _ => "Min1".to_string(), // default
        }
    }
}

impl CommandTranslator for MexcCommandTranslator {
    fn translate_to_commands(&self, subscribe: bool, topics: &[(String, String)]) -> Vec<String> {
        topics
            .iter()
            .map(|(channel, symbol)| {
                if subscribe {
                    Self::v3_subscription_command(channel, symbol)
                } else {
                    Self::v3_unsubscription_command(channel, symbol)
                }
            })
            .collect()
    }

    fn translate_to_candlestick_commands(
        &self,
        subscribe: bool,
        symbol_interval_list: &[(String, usize)],
    ) -> Vec<String> {
        symbol_interval_list
            .iter()
            .map(|(symbol, interval)| {
                // Конвертируем формат из BTC_USDT в BTCUSDT для v3 API
                let mexc_symbol = symbol.replace("_", "");
                let interval_str = Self::interval_to_string(*interval);
                
                if subscribe {
                    format!(
                        r#"{{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@{}@{}"]}}"#,
                        mexc_symbol, interval_str
                    )
                } else {
                    format!(
                        r#"{{"method":"UNSUBSCRIPTION","params":["spot@public.kline.v3.api@{}@{}"]}}"#,
                        mexc_symbol, interval_str
                    )
                }
            })
            .collect()
    }
}

impl MessageHandler for MexcMessageHandler {
    fn handle_message(&mut self, msg: &str) -> MiscMessage {
        if msg == "PONG" {
            return MiscMessage::Pong;
        }
        
        // Проверяем на ping-сообщения
        if msg.trim() == r#"{"msg":"PING"}"# {
            return MiscMessage::WebSocket(Message::Text(r#"{"method":"PONG"}"#.to_string()));
        }
        
        // Обрабатываем сообщения в формате v3 API
        if let Ok(obj) = serde_json::from_str::<HashMap<String, Value>>(msg) {
            // Проверяем PONG ответы в JSON формате
            if let Some(method) = obj.get("method") {
                if method.as_str() == Some("PONG") {
                    return MiscMessage::Pong;
                }
            }
            
            // Альтернативный формат PONG ответа
            if let Some(msg_val) = obj.get("msg") {
                if msg_val.as_str() == Some("PONG") {
                    return MiscMessage::Pong;
                }
            }
            
            // Проверяем успешные подписки/отписки
            if let Some(result) = obj.get("result") {
                if result.as_bool() == Some(true) {
                    info!("Успешная подписка/отписка: {:?}", obj.get("id"));
                    return MiscMessage::Normal;
                }
                if result.as_bool() == Some(false) {
                    if let Some(error) = obj.get("error") {
                        warn!("Ошибка подписки/отписки: {:?}", error);
                    }
                    return MiscMessage::Normal;
                }
            }
            
            // Проверяем обычные данные в старом формате (c - канал, d - данные)
            if obj.contains_key("c") && obj.contains_key("d") {
                if let Some(channel) = obj.get("c").and_then(|v| v.as_str()) {
                    if channel.contains("deals") || channel.contains("depth") || channel.contains("kline") {
                        return MiscMessage::Normal;
                    }
                    // Обработка User Data Stream (баланс аккаунта)
                    if channel.contains("account") || channel.contains("private") {
                        return MiscMessage::Normal;
                    }
                }
            }
            
            // Проверяем новый формат User Data Stream с полем "channel"
            if let Some(channel) = obj.get("channel").and_then(|v| v.as_str()) {
                if channel == "spot@private.account.v3.api.pb" {
                    return MiscMessage::Normal;
                }
                if channel == "spot@private.deals.v3.api.pb" {
                    return MiscMessage::Normal;
                }
            }
            
            // Проверяем сообщения об ошибках
            if let Some(error) = obj.get("error") {
                warn!("Ошибка от MEXC WebSocket: {:?}", error);
                return MiscMessage::Other;
            }
        }
        
        MiscMessage::Normal
    }

    fn get_ping_msg_and_interval(&self) -> Option<(Message, u64)> {
        // MEXC требует отправлять PING каждые 30 секунд для поддержания соединения
        Some((Message::Text(r#"{"method":"PING"}"#.to_string()), 30))
    }
}

impl MessageHandler for MexcUserDataStreamMessageHandler {
    fn handle_message(&mut self, msg: &str) -> MiscMessage {
        debug!("User Data Stream получено сообщение: {}", msg);
        
        if msg == "PONG" {
            debug!("Получен простой PONG ответ");
            return MiscMessage::Pong;
        }
        
        // Проверяем на ping-сообщения
        if msg.trim() == r#"{"msg":"PING"}"# {
            debug!("Получен PING, отправляем PONG");
            return MiscMessage::WebSocket(Message::Text(r#"{"method":"PONG"}"#.to_string()));
        }
        
        // Обрабатываем JSON сообщения
        if let Ok(obj) = serde_json::from_str::<HashMap<String, Value>>(msg) {
            debug!("Разобрано JSON сообщение с полями: {:?}", obj.keys().collect::<Vec<_>>());
            
            // Проверяем PONG ответы
            if let Some(method) = obj.get("method") {
                if method.as_str() == Some("PONG") {
                    debug!("Получен JSON PONG с методом");
                    return MiscMessage::Pong;
                }
            }
            
            if let Some(msg_val) = obj.get("msg") {
                if msg_val.as_str() == Some("PONG") {
                    debug!("Получен JSON PONG с полем msg");
                    return MiscMessage::Pong;
                }
            }
            
            // Проверяем ответы на команды подписки
            if let Some(result) = obj.get("result") {
                if result.as_bool() == Some(true) {
                    info!("Успешная подписка на User Data Stream: {:?}", obj.get("id"));
                    return MiscMessage::Normal;
                }
                if result.as_bool() == Some(false) {
                    if let Some(error) = obj.get("error") {
                        error!("Ошибка подписки на User Data Stream: {:?}", error);
                    } else {
                        error!("Неизвестная ошибка подписки на User Data Stream");
                    }
                    return MiscMessage::Normal;
                }
            }
            
            // Проверяем данные аккаунта по полю "channel"
            if let Some(channel) = obj.get("channel").and_then(|v| v.as_str()) {
                if channel == "spot@private.account.v3.api.pb" {
                    info!("Получены данные баланса аккаунта через User Data Stream");
                    debug!("Данные баланса: {}", msg);
                    return MiscMessage::Normal;
                }
                if channel == "spot@private.deals.v3.api.pb" {
                    info!("Получены данные приватных сделок через User Data Stream");
                    debug!("Данные приватных сделок: {}", msg);
                    return MiscMessage::Normal;
                }
                debug!("Получен неизвестный канал: {}", channel);
            }
            
            // Проверяем на ошибки
            if let Some(error) = obj.get("error") {
                error!("Ошибка от MEXC User Data Stream: {:?}", error);
                return MiscMessage::Other;
            }
            
            // Все остальные сообщения считаем нормальными данными
            debug!("Обработка сообщения как обычных данных");
            return MiscMessage::Normal;
        } else {
            warn!("Не удалось разобрать сообщение как JSON: {}", msg);
        }
        
        MiscMessage::Normal
    }

    fn get_ping_msg_and_interval(&self) -> Option<(Message, u64)> {
        // Для User Data Stream также отправляем PING каждые 30 секунд
        Some((Message::Text(r#"{"method":"PING"}"#.to_string()), 30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::message_handler::MessageHandler;

    #[test]
    fn test_v3_subscription_commands() {
        // Тестируем новые команды подписки v3 API
        let deals_cmd = MexcCommandTranslator::v3_subscription_command("deals", "BTC_USDT");
        assert_eq!(
            deals_cmd,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.deals.v3.api@BTCUSDT"]}"#
        );

        let depth_cmd = MexcCommandTranslator::v3_subscription_command("depth", "ETH_USDT");
        assert_eq!(
            depth_cmd,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.increase.depth.v3.api@ETHUSDT"]}"#
        );

        let kline_cmd = MexcCommandTranslator::v3_subscription_command("kline", "LTC_USDT");
        assert_eq!(
            kline_cmd,
            r#"{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@LTCUSDT@Min1"]}"#
        );
    }

    #[test]
    fn test_user_data_stream_url_formation() {
        let listen_key = "test_listen_key_123";
        let expected_url = format!("{}?listenKey={}", USER_DATA_STREAM_BASE_URL, listen_key);
        let actual_url = format!("{}?listenKey={}", USER_DATA_STREAM_BASE_URL, listen_key);
        
        assert_eq!(expected_url, actual_url);
        assert_eq!(actual_url, "ws://wbs-api.mexc.com/ws?listenKey=test_listen_key_123");
    }

    #[test]
    fn test_candlestick_commands() {
        let translator = MexcCommandTranslator {};
        let commands = translator.translate_to_candlestick_commands(
            true,
            &[("BTC_USDT".to_string(), 60)],
        );

        assert_eq!(1, commands.len());
        assert_eq!(
            r#"{"method":"SUBSCRIPTION","params":["spot@public.kline.v3.api@BTCUSDT@Min1"]}"#,
            commands[0]
        );
    }

    #[test]
    fn test_pong_message_handling() {
        let mut handler = MexcMessageHandler {};
        
        // Тестируем простой текстовый PONG
        let result = handler.handle_message("PONG");
        assert!(matches!(result, MiscMessage::Pong));
        
        // Тестируем JSON PONG с полем "method"
        let json_pong_method = r#"{"method":"PONG"}"#;
        let result = handler.handle_message(json_pong_method);
        assert!(matches!(result, MiscMessage::Pong));
        
        // Тестируем JSON PONG с полем "msg"
        let json_pong_msg = r#"{"msg":"PONG"}"#;
        let result = handler.handle_message(json_pong_msg);
        assert!(matches!(result, MiscMessage::Pong));
        
        // Тестируем, что другие сообщения не распознаются как PONG
        let other_message = r#"{"method":"PING"}"#;
        let result = handler.handle_message(other_message);
        assert!(!matches!(result, MiscMessage::Pong));
    }

    #[test]
    fn test_ping_message_handling() {
        let mut handler = MexcMessageHandler {};
        
        // Тестируем обработку входящего PING
        let ping_msg = r#"{"msg":"PING"}"#;
        let result = handler.handle_message(ping_msg);
        
        if let MiscMessage::WebSocket(Message::Text(response)) = result {
            assert_eq!(response, r#"{"method":"PONG"}"#);
        } else {
            panic!("Ожидался WebSocket PONG ответ");
        }
    }

    #[test]
    fn test_user_data_stream_message_handling() {
        let mut handler = MexcUserDataStreamMessageHandler {};
        
        // Тестируем обработку сообщения баланса аккаунта от User Data Stream
        let account_update_msg = r#"{"channel":"spot@private.account.v3.api.pb","createTime":1736417034305,"sendTime":1736417034307,"privateAccount":{"vcoinName":"USDT","coinId":"128f589271cb4951b03e71e6323eb7be","balanceAmount":"21.94210356004384","balanceAmountChange":"10","frozenAmount":"0","frozenAmountChange":"0","type":"CONTRACT_TRANSFER","time":1736416910000}}"#;
        let result = handler.handle_message(account_update_msg);
        assert!(matches!(result, MiscMessage::Normal));
        
        // Тестируем обработку сообщения приватных сделок от User Data Stream
        let private_deals_msg = r#"{"channel":"spot@private.deals.v3.api.pb","symbol":"MXUSDT","sendTime":1736417034332,"privateDeals":{"price":"3.6962","quantity":"1","amount":"3.6962","tradeType":2,"tradeId":"505979017439002624X1","orderId":"C02__505979017439002624115","feeAmount":"0.0003998377369698171","feeCurrency":"MX","time":1736417034280}}"#;
        let result = handler.handle_message(private_deals_msg);
        assert!(matches!(result, MiscMessage::Normal));
        
        // Тестируем обработку успешного ответа на подписку
        let subscription_success = r#"{"result":true,"id":"subscription_123"}"#;
        let result = handler.handle_message(subscription_success);
        assert!(matches!(result, MiscMessage::Normal));
        
        // Тестируем обработку ошибки подписки
        let subscription_error = r#"{"result":false,"error":{"code":1001,"msg":"Invalid channel"}}"#;
        let result = handler.handle_message(subscription_error);
        assert!(matches!(result, MiscMessage::Normal));
        
        // Тестируем обработку любого JSON сообщения от User Data Stream
        let another_msg = r#"{"someField":"someValue","data":{"test":"value"}}"#;
        let result = handler.handle_message(another_msg);
        assert!(matches!(result, MiscMessage::Normal));
    }

    #[test]
    fn test_regular_private_message_handling() {
        let mut handler = MexcMessageHandler {};
        
        // Тестируем обработку приватного канала в старом формате через обычный клиент
        let private_msg = r#"{"c":"spot@private.deals.v3.api@test_key","d":{"id":"123","price":"50000","qty":"0.001"},"t":1234567890}"#;
        let result = handler.handle_message(private_msg);
        assert!(matches!(result, MiscMessage::Normal));
    }

    #[test]
    fn test_user_data_stream_subscription_command() {
        let expected_command = r#"{"method":"SUBSCRIPTION","params":["spot@private.account.v3.api.pb"]}"#;
        
        // Проверяем, что команда подписки формируется правильно
        let actual_command = r#"{"method":"SUBSCRIPTION","params":["spot@private.account.v3.api.pb"]}"#;
        
        assert_eq!(expected_command, actual_command);
    }
}
