#[macro_use]
mod utils;

pub(crate) mod binance_inverse;
pub(crate) mod binance_linear;
pub(crate) mod binance_option;
pub(crate) mod binance_spot;

use crate::error::Result;
use crypto_market_type::MarketType;

pub(crate) async fn fetch_l2_snapshot(market_type: MarketType, symbol: &str) -> Result<String> {
    match market_type {
        MarketType::Spot => binance_spot::BinanceSpotRestClient::fetch_l2_snapshot(symbol).await,
        MarketType::InverseFuture | MarketType::InverseSwap => {
            binance_inverse::BinanceInverseRestClient::fetch_l2_snapshot(symbol).await
        }
        MarketType::LinearFuture | MarketType::LinearSwap => {
            binance_linear::BinanceLinearRestClient::fetch_l2_snapshot(symbol).await
        }
        MarketType::EuropeanOption => {
            binance_option::BinanceOptionRestClient::fetch_l2_snapshot(symbol).await
        }
        _ => panic!("Binance unknown market_type: {market_type}"),
    }
}

pub(crate) async fn fetch_open_interest(market_type: MarketType, symbol: &str) -> Result<String> {
    match market_type {
        MarketType::InverseFuture | MarketType::InverseSwap => {
            binance_inverse::BinanceInverseRestClient::fetch_open_interest(symbol).await
        }
        MarketType::LinearFuture | MarketType::LinearSwap => {
            binance_linear::BinanceLinearRestClient::fetch_open_interest(symbol).await
        }
        _ => panic!("Binance {market_type} does not have open interest data"),
    }
}
