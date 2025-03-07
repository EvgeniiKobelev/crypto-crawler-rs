use async_trait::async_trait;

use crate::{
    clients::common_traits::{
        Candlestick, Level3OrderBook, OrderBook, OrderBookTopK, Ticker, Trade, BBO,
    },
    common::{command_translator::CommandTranslator, ws_client_internal::WSClientInternal},
    WSClient,
};

use super::{
    utils::{BitgetCommandTranslator, BitgetMessageHandler, UPLINK_LIMIT},
    EXCHANGE_NAME,
};

const WEBSOCKET_URL: &str = "wss://ws.bitget.com/mix/v1/stream";

/// The WebSocket client for Bitget swap markets.
///
/// * WebSocket API doc: <https://bitgetlimited.github.io/apidoc/en/mix/#websocketapi>
/// * Trading at: <https://www.bitget.com/en/swap/>
pub struct BitgetSwapWSClient {
    client: WSClientInternal<BitgetMessageHandler>,
    translator: BitgetCommandTranslator<'M'>,
}

impl BitgetSwapWSClient {
    pub async fn new(tx: std::sync::mpsc::Sender<String>, url: Option<&str>) -> Self {
        let real_url = match url {
            Some(endpoint) => endpoint,
            None => WEBSOCKET_URL,
        };
        BitgetSwapWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                real_url,
                BitgetMessageHandler {},
                Some(UPLINK_LIMIT),
                tx,
            )
            .await,
            translator: BitgetCommandTranslator::<'M'> {},
        }
    }

    /// Create a new BitgetSwapWSClient with a proxy string that includes authentication.
    ///
    /// * `tx` - The channel to send messages to.
    /// * `url` - Optional WebSocket URL, if None, use the default URL.
    /// * `proxy_string` - The proxy string in format "socks5://host:port:username:password".
    pub async fn new_with_proxy(
        tx: std::sync::mpsc::Sender<String>,
        url: Option<&str>,
        proxy_string: &str,
    ) -> Self {
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
        
        let client = BitgetSwapWSClient {
            client: WSClientInternal::connect(
                EXCHANGE_NAME,
                real_url,
                BitgetMessageHandler {},
                Some(UPLINK_LIMIT),
                tx,
            )
            .await,
            translator: BitgetCommandTranslator::<'M'> {},
        };
        
        // Очищаем переменную окружения, чтобы не влиять на другие соединения
        std::env::remove_var("https_proxy");
        std::env::remove_var("https_proxy_username");
        std::env::remove_var("https_proxy_password");
        
        client
    }
}

impl_trait!(Trade, BitgetSwapWSClient, subscribe_trade, "trade");
#[rustfmt::skip]
impl_trait!(OrderBookTopK, BitgetSwapWSClient, subscribe_orderbook_topk, "books15");
impl_trait!(OrderBook, BitgetSwapWSClient, subscribe_orderbook, "books");
impl_trait!(Ticker, BitgetSwapWSClient, subscribe_ticker, "ticker");
impl_candlestick!(BitgetSwapWSClient);

panic_bbo!(BitgetSwapWSClient);
panic_l3_orderbook!(BitgetSwapWSClient);

impl_ws_client_trait!(BitgetSwapWSClient);
