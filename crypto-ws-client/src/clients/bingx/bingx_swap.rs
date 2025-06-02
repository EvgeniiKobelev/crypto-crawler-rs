use async_trait::async_trait;
use log::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

use crate::WSClient;
use crate::common::command_translator::CommandTranslator;
use crate::common::message_handler::{MessageHandler, MiscMessage};
use crate::common::ws_client_internal::WSClientInternal;

const EXCHANGE_NAME: &str = "bingx";

pub(super) const SWAP_WEBSOCKET_URL: &str = "wss://open-api-swap.bingx.com/swap-market";

/// BingX Swap market WebSocket client.
///
/// * WebSocket API doc: <https://bingx-api.github.io/docs/swap-api.html#websocket-market-streams>
/// * Trading at: <https://bingx.com/en-us/futures/>
pub struct BingxSwapWSClient {
    client: WSClientInternal<BingxMessageHandler>,
}

#[derive(Clone)]
pub struct BingxMessageHandler {}

#[derive(Clone)]
pub struct BingxCommandTranslator {}

impl BingxSwapWSClient {
    pub async fn new(tx: Sender<String>, _proxy: Option<String>) -> BingxSwapWSClient {
        BingxSwapWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                SWAP_WEBSOCKET_URL,
                BingxMessageHandler {},
                None,
                tx,
            )
            .await,
        }
    }
}

#[async_trait]
impl WSClient for BingxSwapWSClient {
    async fn subscribe_trade(&self, symbols: &[String]) {
        let commands = symbols
            .iter()
            .map(|symbol| BingxCommandTranslator::subscription_command("trade", symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe_bbo(&self, symbols: &[String]) {
        warn!("BBO не поддерживается для BingX Swap, подписываемся на depth вместо этого");
        self.subscribe_orderbook(symbols).await;
    }

    async fn subscribe_orderbook(&self, symbols: &[String]) {
        let commands = symbols
            .iter()
            .map(|symbol| BingxCommandTranslator::subscription_command("depth", symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe_orderbook_topk(&self, symbols: &[String]) {
        // Для BingX используем тот же канал depth
        self.subscribe_orderbook(symbols).await;
    }

    async fn subscribe_l3_orderbook(&self, _symbols: &[String]) {
        panic!("BingX не поддерживает level3 orderbook");
    }

    async fn subscribe_ticker(&self, symbols: &[String]) {
        let commands = symbols
            .iter()
            .map(|symbol| BingxCommandTranslator::subscription_command("ticker", symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe_candlestick(&self, symbol_interval_list: &[(String, usize)]) {
        let commands = symbol_interval_list
            .iter()
            .map(|(symbol, interval)| {
                let bingx_symbol = symbol.replace("_", "-"); // BTC_USDT -> BTC-USDT
                let interval_str = BingxCommandTranslator::interval_to_string(*interval);
                format!(
                    r#"{{"id":"{}","dataType":"{}@{}.{}"}}"#,
                    chrono::Utc::now().timestamp_millis(),
                    bingx_symbol,
                    "kline",
                    interval_str
                )
            })
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn subscribe(&self, topics: &[(String, String)]) {
        let commands = topics
            .iter()
            .map(|(channel, symbol)| BingxCommandTranslator::subscription_command(channel, symbol))
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn unsubscribe(&self, topics: &[(String, String)]) {
        let commands = topics
            .iter()
            .map(|(channel, symbol)| {
                BingxCommandTranslator::unsubscription_command(channel, symbol)
            })
            .collect::<Vec<_>>();

        self.client.send(&commands).await;
    }

    async fn send(&self, commands: &[String]) {
        self.client.send(commands).await;
    }

    async fn run(&self) {
        self.client.run().await;
    }

    async fn close(&self) {
        self.client.close().await;
    }
}

impl BingxCommandTranslator {
    fn subscription_command(channel: &str, symbol: &str) -> String {
        // Конвертируем символ из формата BTC_USDT в BTC-USDT для BingX
        let bingx_symbol = symbol.replace("_", "-");

        // Формируем команду подписки в формате BingX Swap WebSocket API
        let data_type = match channel {
            "trade" => format!("{}@trade", bingx_symbol),
            "depth" => format!("{}@depth", bingx_symbol),
            "ticker" => format!("{}@ticker", bingx_symbol),
            "kline" => format!("{}@kline_1m", bingx_symbol), // по умолчанию 1 минута
            "markPrice" => format!("{}@markPrice", bingx_symbol), // mark price для фьючерсов
            "fundingRate" => format!("{}@fundingRate", bingx_symbol), // funding rate
            _ => {
                warn!("Неизвестный канал: {}", channel);
                format!("{}@trade", bingx_symbol)
            }
        };

        format!(
            r#"{{"id":"{}","dataType":"{}"}}"#,
            chrono::Utc::now().timestamp_millis(),
            data_type
        )
    }

    fn unsubscription_command(channel: &str, symbol: &str) -> String {
        // Конвертируем символ из формата BTC_USDT в BTC-USDT для BingX
        let bingx_symbol = symbol.replace("_", "-");

        // Формируем команду отписки
        let data_type = match channel {
            "trade" => format!("{}@trade", bingx_symbol),
            "depth" => format!("{}@depth", bingx_symbol),
            "ticker" => format!("{}@ticker", bingx_symbol),
            "kline" => format!("{}@kline_1m", bingx_symbol),
            "markPrice" => format!("{}@markPrice", bingx_symbol),
            "fundingRate" => format!("{}@fundingRate", bingx_symbol),
            _ => {
                warn!("Неизвестный канал: {}", channel);
                format!("{}@trade", bingx_symbol)
            }
        };

        format!(
            r#"{{"id":"{}","dataType":"{}","unsubscribe":true}}"#,
            chrono::Utc::now().timestamp_millis(),
            data_type
        )
    }

    fn interval_to_string(interval: usize) -> String {
        match interval {
            60 => "1m".to_string(),
            300 => "5m".to_string(),
            900 => "15m".to_string(),
            1800 => "30m".to_string(),
            3600 => "1h".to_string(),
            7200 => "2h".to_string(),
            14400 => "4h".to_string(),
            21600 => "6h".to_string(),
            28800 => "8h".to_string(),
            43200 => "12h".to_string(),
            86400 => "1d".to_string(),
            259200 => "3d".to_string(),
            604800 => "1w".to_string(),
            2592000 => "1M".to_string(),
            _ => "1m".to_string(), // по умолчанию
        }
    }
}

impl CommandTranslator for BingxCommandTranslator {
    fn translate_to_commands(&self, subscribe: bool, topics: &[(String, String)]) -> Vec<String> {
        topics
            .iter()
            .map(|(channel, symbol)| {
                if subscribe {
                    Self::subscription_command(channel, symbol)
                } else {
                    Self::unsubscription_command(channel, symbol)
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
                // Конвертируем формат из BTC_USDT в BTC-USDT для BingX
                let bingx_symbol = symbol.replace("_", "-");
                let interval_str = Self::interval_to_string(*interval);

                if subscribe {
                    format!(
                        r#"{{"id":"{}","dataType":"{}@kline.{}"}}"#,
                        chrono::Utc::now().timestamp_millis(),
                        bingx_symbol,
                        interval_str
                    )
                } else {
                    format!(
                        r#"{{"id":"{}","dataType":"{}@kline.{}","unsubscribe":true}}"#,
                        chrono::Utc::now().timestamp_millis(),
                        bingx_symbol,
                        interval_str
                    )
                }
            })
            .collect()
    }
}

impl MessageHandler for BingxMessageHandler {
    fn handle_message(&mut self, msg: &str) -> MiscMessage {
        if msg.trim() == "Ping" {
            return MiscMessage::WebSocket(Message::Text("Pong".to_string()));
        }

        if msg.trim() == "Pong" {
            return MiscMessage::Pong;
        }

        // Обрабатываем JSON сообщения
        if let Ok(obj) = serde_json::from_str::<HashMap<String, Value>>(msg) {
            // Проверяем ping сообщения в JSON формате
            if let Some(ping) = obj.get("ping") {
                return MiscMessage::WebSocket(Message::Text(format!(
                    r#"{{"pong":{}}}"#,
                    ping.as_u64().unwrap_or(0)
                )));
            }

            // Проверяем pong ответы
            if obj.contains_key("pong") {
                return MiscMessage::Pong;
            }

            // Проверяем ответы на подписку/отписку
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

            // Проверяем обычные данные (содержат dataType и data)
            if obj.contains_key("dataType") && obj.contains_key("data") {
                return MiscMessage::Normal;
            }

            // Проверяем сообщения об ошибках
            if let Some(error) = obj.get("error") {
                warn!("Ошибка от BingX Swap WebSocket: {:?}", error);
                return MiscMessage::Other;
            }
        }

        MiscMessage::Normal
    }

    fn get_ping_msg_and_interval(&self) -> Option<(Message, u64)> {
        // BingX требует отправлять ping каждые 30 секунд
        Some((Message::Text("Ping".to_string()), 30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_commands() {
        let trade_cmd = BingxCommandTranslator::subscription_command("trade", "BTC_USDT");
        assert!(trade_cmd.contains("BTC-USDT@trade"));
        assert!(trade_cmd.contains("\"id\":"));
        assert!(trade_cmd.contains("\"dataType\":"));

        let depth_cmd = BingxCommandTranslator::subscription_command("depth", "ETH_USDT");
        assert!(depth_cmd.contains("ETH-USDT@depth"));

        let ticker_cmd = BingxCommandTranslator::subscription_command("ticker", "LTC_USDT");
        assert!(ticker_cmd.contains("LTC-USDT@ticker"));

        // Тестируем специфичные для swap каналы
        let mark_price_cmd = BingxCommandTranslator::subscription_command("markPrice", "BTC_USDT");
        assert!(mark_price_cmd.contains("BTC-USDT@markPrice"));

        let funding_rate_cmd =
            BingxCommandTranslator::subscription_command("fundingRate", "BTC_USDT");
        assert!(funding_rate_cmd.contains("BTC-USDT@fundingRate"));
    }

    #[test]
    fn test_candlestick_commands() {
        let translator = BingxCommandTranslator {};
        let commands =
            translator.translate_to_candlestick_commands(true, &[("BTC_USDT".to_string(), 60)]);

        assert_eq!(1, commands.len());
        assert!(commands[0].contains("BTC-USDT@kline.1m"));
    }

    #[test]
    fn test_message_handling() {
        let mut handler = BingxMessageHandler {};

        // Тестируем простой текстовый ping/pong
        let result = handler.handle_message("Ping");
        if let MiscMessage::WebSocket(Message::Text(response)) = result {
            assert_eq!(response, "Pong");
        } else {
            panic!("Ожидался WebSocket Pong ответ");
        }

        let result = handler.handle_message("Pong");
        assert!(matches!(result, MiscMessage::Pong));

        // Тестируем JSON ping
        let ping_json = r#"{"ping":1234567890}"#;
        let result = handler.handle_message(ping_json);
        if let MiscMessage::WebSocket(Message::Text(response)) = result {
            assert_eq!(response, r#"{"pong":1234567890}"#);
        } else {
            panic!("Ожидался WebSocket pong ответ");
        }
    }

    #[test]
    fn test_interval_conversion() {
        assert_eq!(BingxCommandTranslator::interval_to_string(60), "1m");
        assert_eq!(BingxCommandTranslator::interval_to_string(300), "5m");
        assert_eq!(BingxCommandTranslator::interval_to_string(3600), "1h");
        assert_eq!(BingxCommandTranslator::interval_to_string(86400), "1d");
        assert_eq!(BingxCommandTranslator::interval_to_string(2592000), "1M");
        assert_eq!(BingxCommandTranslator::interval_to_string(999), "1m"); // default
    }
}
