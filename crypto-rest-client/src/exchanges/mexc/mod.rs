pub(crate) mod mexc_spot;
pub(crate) mod mexc_swap;

use crate::error::Result;
use crypto_market_type::MarketType;

pub(crate) async fn fetch_l2_snapshot(market_type: MarketType, symbol: &str) -> Result<String> {
    match market_type {
        MarketType::Spot => mexc_spot::MexcSpotRestClient::fetch_l2_snapshot(symbol).await,
        MarketType::InverseSwap | MarketType::LinearSwap => {
            mexc_swap::MexcSwapRestClient::fetch_l2_snapshot(symbol).await
        }
        _ => panic!("MEXC unknown market_type: {market_type}"),
    }
}
