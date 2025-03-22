use async_trait::async_trait;

use crate::{
    clients::common_traits::{
        Candlestick, Level3OrderBook, OrderBook, OrderBookTopK, Ticker, Trade, BBO,
    },
    common::{command_translator::CommandTranslator, ws_client_internal::WSClientInternal},
    WSClient,
};

use super::utils::{BybitMessageHandler, EXCHANGE_NAME, UPLINK_LIMIT};

// URL для API v5 Spot
const WEBSOCKET_URL: &str = "wss://stream.bybit.com/v5/public/spot";

/// Bybit Spot market.
///
/// * WebSocket API doc: <https://bybit-exchange.github.io/docs/v5/websocket/public/trade>
/// * Trading at: <https://www.bybit.com/trade/spot/>
pub struct BybitSpotWSClient {
    client: WSClientInternal<BybitMessageHandler>,
    translator: BybitSpotCommandTranslator,
}

impl_new_constructor!(
    BybitSpotWSClient,
    EXCHANGE_NAME,
    WEBSOCKET_URL,
    BybitMessageHandler {},
    BybitSpotCommandTranslator {}
);

impl BybitSpotWSClient {
    pub async fn new_with_proxy(tx: std::sync::mpsc::Sender<String>, url: Option<&str>, proxy_string: &str) -> Self {
        // Используем прокси в формате socks5://a7W4HM:0BFYrV@45.81.77.174:8000
        let real_url = match url {
            Some(endpoint) => endpoint,
            None => WEBSOCKET_URL,
        };
        
        // Устанавливаем переменную окружения для прокси
        std::env::set_var("https_proxy", proxy_string);
        
        let client = BybitSpotWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                real_url,
                BybitMessageHandler {},
                Some(UPLINK_LIMIT),
                tx,
            )
            .await,
            translator: BybitSpotCommandTranslator {},
        };
        
        // Очищаем переменную окружения, чтобы не влиять на другие соединения
        std::env::remove_var("https_proxy");
        
        client
    }
}

impl_trait!(Trade, BybitSpotWSClient, subscribe_trade, "trade");
#[rustfmt::skip]
// В API v5 используется orderbook.1 для актуального ордербука
impl_trait!(OrderBook, BybitSpotWSClient, subscribe_orderbook, "orderbook.1");
#[rustfmt::skip]
// В API v5 используется tickers для получения сводной информации
impl_trait!(Ticker, BybitSpotWSClient, subscribe_ticker, "tickers");
impl_candlestick!(BybitSpotWSClient);
panic_bbo!(BybitSpotWSClient);
panic_l3_orderbook!(BybitSpotWSClient);
panic_l2_topk!(BybitSpotWSClient);

impl_ws_client_trait!(BybitSpotWSClient);

struct BybitSpotCommandTranslator {}

impl BybitSpotCommandTranslator {
    // https://bybit-exchange.github.io/docs/v5/websocket/public/kline
    fn to_candlestick_raw_channel(interval: usize) -> String {
        let interval_str = match interval {
            60 => "1",
            180 => "3",
            300 => "5",
            900 => "15",
            1800 => "30",
            3600 => "60",
            7200 => "120",
            14400 => "240",
            21600 => "360",
            86400 => "D",
            604800 => "W",
            2592000 => "M",
            _ => panic!(
                "Bybit Spot has intervals 1min,3min,5min,15min,30min,60min,120min,240min,360min,1day,1week,1month"
            ),
        };
        // В API v5 используется kline.{interval} вместо candle.{interval}
        format!("kline.{interval_str}")
    }
}

impl CommandTranslator for BybitSpotCommandTranslator {
    fn translate_to_commands(&self, subscribe: bool, topics: &[(String, String)]) -> Vec<String> {
        vec![super::utils::topics_to_command(topics, subscribe)]
    }

    fn translate_to_candlestick_commands(
        &self,
        subscribe: bool,
        symbol_interval_list: &[(String, usize)],
    ) -> Vec<String> {
        let topics = symbol_interval_list
            .iter()
            .map(|(symbol, interval)| {
                let channel = Self::to_candlestick_raw_channel(*interval);
                (channel, symbol.to_string())
            })
            .collect::<Vec<(String, String)>>();
        self.translate_to_commands(subscribe, &topics)
    }
}

