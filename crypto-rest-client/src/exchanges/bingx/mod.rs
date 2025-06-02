mod bingx_spot;
mod bingx_swap;

pub use bingx_spot::BingxSpotRestClient;
pub use bingx_swap::BingxSwapRestClient;

use crate::error::Result;
use crypto_market_type::MarketType;

pub(crate) async fn fetch_l2_snapshot(market_type: MarketType, symbol: &str) -> Result<String> {
    match market_type {
        MarketType::Spot => bingx_spot::BingxSpotRestClient::fetch_l2_snapshot(symbol).await,
        MarketType::LinearSwap => bingx_swap::BingxSwapRestClient::fetch_l2_snapshot(symbol).await,
        _ => panic!("BingX unknown market_type: {market_type}"),
    }
}

pub(crate) async fn fetch_open_interest(market_type: MarketType, symbol: &str) -> Result<String> {
    match market_type {
        MarketType::LinearSwap => {
            bingx_swap::BingxSwapRestClient::fetch_open_interest(symbol).await
        }
        _ => panic!("BingX {market_type} does not have open interest data"),
    }
}
