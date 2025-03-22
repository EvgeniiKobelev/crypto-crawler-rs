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

    /// Create a new BitgetSwapWSClient with a proxy string.
    ///
    /// * `tx` - The channel to send messages to.
    /// * `url` - Optional WebSocket URL, if None, use the default URL.
    /// * `proxy_string` - The proxy string in format "socks5://username:password@host:port".
    pub async fn new_with_proxy(
        tx: std::sync::mpsc::Sender<String>,
        url: Option<&str>,
        proxy_string: &str,
    ) -> Self {
        let real_url = match url {
            Some(endpoint) => endpoint,
            None => WEBSOCKET_URL,
        };
        
        // Устанавливаем переменную окружения для прокси
        std::env::set_var("https_proxy", proxy_string);
        
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
        
        client
    }
    
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

    /// Создание ордера на рынке Bitget Swap
    ///
    /// # Аргументы
    ///
    /// * `symbol` - Символ торговой пары, например "BTCUSDT_UMCBL"
    /// * `side` - Сторона ордера: "buy" или "sell"
    /// * `order_type` - Тип ордера: "limit", "market", "stop" и т.д.
    /// * `quantity` - Количество базовой валюты
    /// * `price` - Цена для лимитного ордера (не обязательна для рыночного ордера)
    /// * `client_order_id` - Необязательный идентификатор ордера клиента
    ///
    /// # Примечание
    ///
    /// Для использования этого метода требуется аутентификация. 
    /// Перед вызовом убедитесь, что у вас есть правильно настроенные ключи API.
    pub async fn create_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: f64,
        price: Option<f64>,
        client_order_id: Option<&str>,
    ) {
        let mut order_data = serde_json::json!({
            "instId": symbol,
            "side": side,
            "ordType": order_type,
            "sz": quantity.to_string(),
        });

        if let Some(p) = price {
            order_data["px"] = serde_json::json!(p.to_string());
        }

        if let Some(id) = client_order_id {
            order_data["clOrdId"] = serde_json::json!(id);
        }

        let command = format!(
            r#"{{"op":"order","args":[{}]}}"#,
            serde_json::to_string(&order_data).unwrap()
        );

        self.client.send(&[command]).await;
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
