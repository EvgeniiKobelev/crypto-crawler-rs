use async_trait::async_trait;

use crate::{
    clients::common_traits::{
        Candlestick, Level3OrderBook, OrderBook, OrderBookTopK, Ticker, Trade, BBO,
    },
    common::{command_translator::CommandTranslator, ws_client_internal::WSClientInternal},
    WSClient,
};

use super::utils::{BybitMessageHandler, EXCHANGE_NAME};

// Обновленный URL для API v5
const WEBSOCKET_URL: &str = "wss://stream.bybit.com/v5/public/inverse";

/// Bybit Inverses markets.
///
/// InverseFuture:
///   * WebSocket API doc: <https://bybit-exchange.github.io/docs/v5/websocket/public/trade>
///   * Trading at: <https://www.bybit.com/trade/inverse/futures/>
///
/// InverseSwap:
///   * WebSocket API doc: <https://bybit-exchange.github.io/docs/v5/websocket/public/trade>
///   * Trading at: <https://www.bybit.com/trade/inverse/>
pub struct BybitInverseWSClient {
    client: WSClientInternal<BybitMessageHandler>,
    translator: BybitInverseCommandTranslator,
}

impl_new_constructor!(
    BybitInverseWSClient,
    EXCHANGE_NAME,
    WEBSOCKET_URL,
    BybitMessageHandler {},
    BybitInverseCommandTranslator {}
);

impl_trait!(Trade, BybitInverseWSClient, subscribe_trade, "trade");
#[rustfmt::skip]
// В API v5 используется orderbook.25 вместо orderBookL2_25
impl_trait!(OrderBook, BybitInverseWSClient, subscribe_orderbook, "orderbook.25");
#[rustfmt::skip]
// В API v5 используется tickers вместо instrument_info.100ms
impl_trait!(Ticker, BybitInverseWSClient, subscribe_ticker, "tickers");
impl_candlestick!(BybitInverseWSClient);
panic_bbo!(BybitInverseWSClient);
panic_l3_orderbook!(BybitInverseWSClient);
panic_l2_topk!(BybitInverseWSClient);

impl_ws_client_trait!(BybitInverseWSClient);

struct BybitInverseCommandTranslator {}

impl BybitInverseCommandTranslator {
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
                "Bybit InverseFuture has intervals 1min,3min,5min,15min,30min,60min,120min,240min,360min,1day,1week,1month"
            ),
        };
        // В API v5 используется kline.{interval} вместо klineV2.{interval}
        format!("kline.{interval_str}")
    }
}

impl CommandTranslator for BybitInverseCommandTranslator {
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
