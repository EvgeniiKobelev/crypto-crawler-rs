use async_trait::async_trait;

use crate::{
    clients::common_traits::{
        Candlestick, Level3OrderBook, OrderBook, OrderBookTopK, Ticker, Trade, BBO,
    },
    common::{command_translator::CommandTranslator, ws_client_internal::WSClientInternal},
    WSClient,
};

use super::utils::{BybitMessageHandler, EXCHANGE_NAME, UPLINK_LIMIT};

// Обновленный URL для API v5
const WEBSOCKET_URL: &str = "wss://stream.bybit.com/v5/public/linear";

/// Bybit LinearSwap market.
///
/// * WebSocket API doc: <https://bybit-exchange.github.io/docs/v5/websocket/public/trade>
/// * Trading at: <https://www.bybit.com/trade/usdt/>
pub struct BybitLinearSwapWSClient {
    client: WSClientInternal<BybitMessageHandler>,
    translator: BybitLinearCommandTranslator,
}

impl_new_constructor!(
    BybitLinearSwapWSClient,
    EXCHANGE_NAME,
    WEBSOCKET_URL,
    BybitMessageHandler {},
    BybitLinearCommandTranslator {}
);

impl BybitLinearSwapWSClient {
    pub async fn new_with_proxy(tx: std::sync::mpsc::Sender<String>, url: Option<&str>, proxy_string: &str) -> Self {
        // Проверяем, содержит ли строка протокол
        if !proxy_string.contains("://") {
            panic!("Invalid proxy string format: {}, expected format: protocol://host:port:username:password", proxy_string);
        }
        
        // Разделяем строку на протокол и остальную часть
        let parts: Vec<&str> = proxy_string.split("://").collect();
        let protocol = parts[0];
        let rest = parts[1];
        
        // Разделяем остальную часть по двоеточию
        let components: Vec<&str> = rest.split(':').collect();
        
        if components.len() < 2 {
            panic!("Invalid proxy string format: {}, expected format: protocol://host:port:username:password", proxy_string);
        }
        
        let host = components[0];
        let port = components[1];
        
        // Формируем базовый URL прокси
        let base_url = format!("{}://{}:{}", protocol, host, port);
        
        // Извлекаем логин и пароль, если они есть
        let (username, password) = if components.len() >= 4 {
            (Some(components[2]), Some(components[3]))
        } else {
            (None, None)
        };
        
        let real_url = match url {
            Some(endpoint) => endpoint,
            None => WEBSOCKET_URL,
        };
        
        // Временно устанавливаем переменную окружения для прокси
        std::env::set_var("https_proxy", &base_url);
        std::env::set_var("https_proxy_username", username.unwrap_or(""));
        std::env::set_var("https_proxy_password", password.unwrap_or(""));
        
        let client = BybitLinearSwapWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                real_url,
                BybitMessageHandler {},
                Some(UPLINK_LIMIT),
                tx,
            )
            .await,
            translator: BybitLinearCommandTranslator {},
        };
        
        // Очищаем переменную окружения, чтобы не влиять на другие соединения
        std::env::remove_var("https_proxy");
        std::env::remove_var("https_proxy_username");
        std::env::remove_var("https_proxy_password");
        
        client
    }
}
impl_trait!(Trade, BybitLinearSwapWSClient, subscribe_trade, "trade");
#[rustfmt::skip]
// В API v5 используется orderbook.25 вместо orderBookL2_25
impl_trait!(OrderBook, BybitLinearSwapWSClient, subscribe_orderbook, "orderbook.25");
#[rustfmt::skip]
// В API v5 используется tickers вместо instrument_info.100ms
impl_trait!(Ticker, BybitLinearSwapWSClient, subscribe_ticker, "tickers");
impl_candlestick!(BybitLinearSwapWSClient);
panic_bbo!(BybitLinearSwapWSClient);
panic_l3_orderbook!(BybitLinearSwapWSClient);
panic_l2_topk!(BybitLinearSwapWSClient);

impl_ws_client_trait!(BybitLinearSwapWSClient);

struct BybitLinearCommandTranslator {}

impl BybitLinearCommandTranslator {
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
                "Bybit LinearSwap has intervals 1min,3min,5min,15min,30min,60min,120min,240min,360min,1day,1week,1month"
            ),
        };
        // В API v5 используется kline.{interval} вместо candle.{interval}
        format!("kline.{interval_str}")
    }
}

impl CommandTranslator for BybitLinearCommandTranslator {
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
