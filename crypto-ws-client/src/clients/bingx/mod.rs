mod bingx_spot;
mod bingx_swap;

pub(super) const EXCHANGE_NAME: &str = "bingx";

pub use bingx_spot::BingxSpotWSClient;
pub use bingx_swap::BingxSwapWSClient;
