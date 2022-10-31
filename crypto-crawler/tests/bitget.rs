#[macro_use]
mod utils;

use test_case::test_case;

use crypto_crawler::*;
use crypto_market_type::MarketType;
use crypto_msg_type::MessageType;
use utils::parse;

const EXCHANGE_NAME: &str = "bitget";

// #[test_case(MarketType::InverseFuture)]
#[test_case(MarketType::InverseSwap)]
#[test_case(MarketType::LinearSwap)]
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_crawl_trade_all(market_type: MarketType) {
    test_all_symbols!(crawl_trade, EXCHANGE_NAME, market_type, MessageType::Trade)
}

// #[test_case(MarketType::InverseFuture, "BTCUSD_DMCBL_221230")]
#[test_case(MarketType::InverseSwap, "BTCUSD_DMCBL")]
#[test_case(MarketType::LinearSwap, "BTCUSDT_UMCBL")]
#[tokio::test(flavor = "multi_thread")]
async fn test_crawl_trade(market_type: MarketType, symbol: &str) {
    test_one_symbol!(
        crawl_trade,
        EXCHANGE_NAME,
        market_type,
        symbol,
        MessageType::Trade
    )
}

// #[test_case(MarketType::InverseFuture, "BTCUSD_DMCBL_221230")]
#[test_case(MarketType::InverseSwap, "BTCUSD_DMCBL")]
#[test_case(MarketType::LinearSwap, "BTCUSDT_UMCBL")]
#[tokio::test(flavor = "multi_thread")]
async fn test_crawl_l2_event(market_type: MarketType, symbol: &str) {
    test_one_symbol!(
        crawl_l2_event,
        EXCHANGE_NAME,
        market_type,
        symbol,
        MessageType::L2Event
    )
}

// #[test_case(MarketType::InverseFuture, "BTCUSD_DMCBL_221230")]
#[test_case(MarketType::InverseSwap, "BTCUSD_DMCBL")]
#[test_case(MarketType::LinearSwap, "BTCUSDT_UMCBL")]
#[tokio::test(flavor = "multi_thread")]
async fn test_crawl_l2_topk(market_type: MarketType, symbol: &str) {
    test_one_symbol!(
        crawl_l2_topk,
        EXCHANGE_NAME,
        market_type,
        symbol,
        MessageType::L2TopK
    )
}

#[test_case(MarketType::Spot, "BTCUSDT_SPBL")]
#[test_case(MarketType::InverseFuture, "BTCUSD_DMCBL_221230")]
#[test_case(MarketType::InverseSwap, "BTCUSD_DMCBL")]
#[test_case(MarketType::LinearSwap, "BTCUSDT_UMCBL")]
fn test_crawl_l2_snapshot(market_type: MarketType, symbol: &str) {
    test_crawl_restful!(
        crawl_l2_snapshot,
        EXCHANGE_NAME,
        market_type,
        symbol,
        MessageType::L2Snapshot
    )
}

#[test_case(MarketType::Spot)]
#[test_case(MarketType::InverseFuture)]
#[test_case(MarketType::InverseSwap)]
#[test_case(MarketType::LinearSwap)]
fn test_crawl_l2_snapshot_without_symbol(market_type: MarketType) {
    test_crawl_restful_all_symbols!(
        crawl_l2_snapshot,
        EXCHANGE_NAME,
        market_type,
        MessageType::L2Snapshot
    )
}

// #[test_case(MarketType::InverseFuture, "BTCUSD_DMCBL_221230")]
#[test_case(MarketType::InverseSwap, "BTCUSD_DMCBL")]
#[test_case(MarketType::LinearSwap, "BTCUSDT_UMCBL")]
#[tokio::test(flavor = "multi_thread")]
async fn test_crawl_ticker(market_type: MarketType, symbol: &str) {
    test_one_symbol!(
        crawl_ticker,
        EXCHANGE_NAME,
        market_type,
        symbol,
        MessageType::Ticker
    )
}

// #[test_case(MarketType::InverseFuture)]
#[test_case(MarketType::InverseSwap)]
#[test_case(MarketType::LinearSwap)]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn test_crawl_candlestick(market_type: MarketType) {
    gen_test_crawl_candlestick!(EXCHANGE_NAME, market_type)
}
