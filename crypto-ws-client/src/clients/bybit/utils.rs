use std::{
    collections::HashMap,
    num::NonZeroU32,
};
use nonzero_ext::nonzero;
use log::*;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

use crate::common::message_handler::{MessageHandler, MiscMessage};

pub(super) const EXCHANGE_NAME: &str = "bybit";

pub(super) fn topics_to_command(topics: &[(String, String)], subscribe: bool) -> String {
    let raw_channels = topics
        .iter()
        .map(|(channel, symbol)| {
            let v5_channel = match channel.as_str() {
                "trade" => "publicTrade",
                "orderBookL2_25" => "orderbook.25",
                "instrument_info.100ms" => "tickers",
                "candle" => "kline",
                _ => channel,
            };
            format!("{v5_channel}.{symbol}")
        })
        .collect::<Vec<String>>();
    format!(
        r#"{{"op":"{}","args":{}}}"#,
        if subscribe { "subscribe" } else { "unsubscribe" },
        serde_json::to_string(&raw_channels).unwrap()
    )
}

// Do not frequently connect and disconnect the connection.
// Do not build over 500 connections in 5 minutes. This is counted per WebSocket domain.
pub(super) const UPLINK_LIMIT: (NonZeroU32, std::time::Duration) =
    (nonzero!(500u32), std::time::Duration::from_secs(300));

pub(super) struct BybitMessageHandler {}

impl MessageHandler for BybitMessageHandler {
    fn handle_message(&mut self, msg: &str) -> MiscMessage {
        let obj = serde_json::from_str::<HashMap<String, Value>>(msg).unwrap();

        if obj.contains_key("topic") && obj.contains_key("data") {
            MiscMessage::Normal
        } else if obj.contains_key("type") && obj.get("type").unwrap().as_str().unwrap() == "snapshot" {
            MiscMessage::Normal
        } else {
            if obj.contains_key("success") {
                if obj.get("success").unwrap().as_bool().unwrap() {
                    info!("Received {} from {}", msg, EXCHANGE_NAME);
                    if obj.contains_key("ret_msg")
                        && obj.get("ret_msg").unwrap().as_str().unwrap() == "pong"
                    {
                        return MiscMessage::Pong;
                    }
                } else {
                    error!("Received {} from {}", msg, EXCHANGE_NAME);
                    warn!("Error in subscription: {msg}");
                    return MiscMessage::Other;
                }
            } else {
                warn!("Received {} from {}", msg, EXCHANGE_NAME);
            }
            MiscMessage::Other
        }
    }

    fn get_ping_msg_and_interval(&self) -> Option<(Message, u64)> {
        // See:
        // - https://bybit-exchange.github.io/docs/inverse/#t-heartbeat
        // - https://bybit-exchange.github.io/docs/linear/#t-heartbeat
        // Обновленный формат пинга для API v5
        Some((Message::Text(r#"{"op":"ping"}"#.to_string()), 30))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_one_channel() {
        let command =
            super::topics_to_command(&[("trade".to_string(), "BTCUSD".to_string())], true);
        assert_eq!(r#"{"op":"subscribe","args":["publicTrade.BTCUSD"]}"#, command);
    }

    #[test]
    fn test_multiple_channels() {
        let command = super::topics_to_command(
            &[
                ("trade".to_string(), "BTCUSD".to_string()),
                ("orderBookL2_25".to_string(), "BTCUSD".to_string()),
                ("instrument_info.100ms".to_string(), "BTCUSD".to_string()),
            ],
            true,
        );

        assert_eq!(
            r#"{"op":"subscribe","args":["publicTrade.BTCUSD","orderbook.25.BTCUSD","tickers.BTCUSD"]}"#,
            command
        );
    }
}
