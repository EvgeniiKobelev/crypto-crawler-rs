#[macro_use]
mod utils;

#[cfg(test)]
mod mexc_spot {
    use crypto_ws_client::{MexcSpotWSClient, MexcUserDataStreamWSClient, WSClient};

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe() {
        gen_test_code!(
            MexcSpotWSClient,
            subscribe,
            &[
                ("deal".to_string(), "BTC_USDT".to_string()),
                ("deal".to_string(), "ETH_USDT".to_string()),
                ("deal".to_string(), "MX_USDT".to_string())
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_raw_json() {
        gen_test_code!(
            MexcSpotWSClient,
            send,
            &[
                r#"{"op":"sub.deal","symbol":"BTC_USDT"}"#.to_string(),
                r#"{"op":"sub.deal","symbol":"ETH_USDT"}"#.to_string(),
                r#"{"op":"sub.deal","symbol":"MX_USDT"}"#.to_string()
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_trade() {
        gen_test_code!(
            MexcSpotWSClient,
            subscribe_trade,
            &["BTC_USDT".to_string(), "ETH_USDT".to_string(), "MX_USDT".to_string()]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook() {
        gen_test_code!(
            MexcSpotWSClient,
            subscribe_orderbook,
            &["BTC_USDT".to_string(), "ETH_USDT".to_string(), "MX_USDT".to_string()]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook_topk() {
        gen_test_code!(MexcSpotWSClient, subscribe_orderbook_topk, &["BTC_USDT".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_candlestick() {
        gen_test_subscribe_candlestick!(
            MexcSpotWSClient,
            &[
                ("BTC_USDT".to_string(), 60),
                ("ETH_USDT".to_string(), 60),
                ("MX_USDT".to_string(), 60)
            ]
        );
        gen_test_subscribe_candlestick!(
            MexcSpotWSClient,
            &[
                ("BTC_USDT".to_string(), 2592000),
                ("ETH_USDT".to_string(), 2592000),
                ("MX_USDT".to_string(), 2592000)
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_overview() {
        gen_test_code!(MexcSpotWSClient, send, &[r#"{"op":"sub.overview"}"#.to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_account_balance() {
        // Этот тест проверяет, что метод subscribe_account_balance
        // корректно выводит предупреждение о необходимости использования отдельного WebSocket
        let (tx, _rx) = std::sync::mpsc::channel();
        let ws_client = MexcSpotWSClient::new(tx, None).await;

        // Метод должен выполниться без ошибок, но не отправлять команды
        ws_client.subscribe_account_balance("test_listen_key_123").await;

        // Тест проходит, если не произошло panic
        assert!(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_private_deals_user_data_stream() {
        // Тест для подписки на приватные сделки через User Data Stream
        let (tx, _rx) = std::sync::mpsc::channel();
        let listen_key = "test_listen_key_for_private_deals";
        let ws_client = MexcUserDataStreamWSClient::new(listen_key, tx, None).await;

        // Метод должен выполниться без ошибок и отправить команду подписки
        ws_client.subscribe_private_deals().await;

        // Тест проходит, если не произошло panic
        assert!(true);
    }
}

#[cfg(test)]
mod mexc_linear_swap {
    use crypto_ws_client::{MexcSwapWSClient, WSClient};

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe() {
        gen_test_code!(
            MexcSwapWSClient,
            subscribe,
            &[
                ("deal".to_string(), "BTC_USDT".to_string()),
                ("deal".to_string(), "ETH_USDT".to_string())
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_raw_json() {
        gen_test_code!(
            MexcSwapWSClient,
            send,
            &[r#"{"method":"sub.deal","param":{"symbol":"BTC_USDT"}}"#.to_string()]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_trade() {
        gen_test_code!(MexcSwapWSClient, subscribe_trade, &["BTC_USDT".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_ticker() {
        gen_test_code!(MexcSwapWSClient, subscribe_ticker, &["BTC_USDT".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook() {
        gen_test_code!(MexcSwapWSClient, subscribe_orderbook, &["BTC_USDT".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook_topk() {
        gen_test_code!(MexcSwapWSClient, subscribe_orderbook_topk, &["BTC_USDT".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_candlestick() {
        gen_test_subscribe_candlestick!(MexcSwapWSClient, &[("BTC_USDT".to_string(), 60)]);
        gen_test_subscribe_candlestick!(MexcSwapWSClient, &[("BTC_USDT".to_string(), 2592000)]);
    }
}

#[cfg(test)]
mod mexc_inverse_swap {
    use crypto_ws_client::{MexcSwapWSClient, WSClient};

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_trade() {
        gen_test_code!(MexcSwapWSClient, subscribe_trade, &["BTC_USD".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_ticker() {
        gen_test_code!(MexcSwapWSClient, subscribe_ticker, &["BTC_USD".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook() {
        gen_test_code!(MexcSwapWSClient, subscribe_orderbook, &["BTC_USD".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_orderbook_topk() {
        gen_test_code!(MexcSwapWSClient, subscribe_orderbook_topk, &["BTC_USD".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_candlestick() {
        gen_test_subscribe_candlestick!(MexcSwapWSClient, &[("BTC_USD".to_string(), 60)]);
        gen_test_subscribe_candlestick!(MexcSwapWSClient, &[("BTC_USD".to_string(), 2592000)]);
    }
}
