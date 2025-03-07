use super::super::utils::http_get;
use crate::error::Result;
use std::collections::BTreeMap;
use serde_json::Value;
use std::time::Duration;
const BASE_URL: &str = "https://api.bitget.com";

/// The RESTful client for Bitget swap markets.
///
/// * RESTful API doc: <https://bitgetlimited.github.io/apidoc/en/mix/#restapi>
/// * Trading at: <https://www.bitget.com/mix/>
pub struct BitgetSwapRestClient {
    _api_key: Option<String>,
    _api_secret: Option<String>,
}

impl BitgetSwapRestClient {
    pub fn new(api_key: Option<String>, api_secret: Option<String>) -> Self {
        BitgetSwapRestClient { _api_key: api_key, _api_secret: api_secret }
    }

    /// Get the latest Level2 snapshot of orderbook.
    ///
    /// For example: <https://api.bitget.com/api/mix/v1/market/depth?symbol=BTCUSDT_UMCBL&limit=100>
    ///
    /// Rate Limit：20 requests per 2 seconds
    pub fn fetch_l2_snapshot(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/mix/v1/market/depth?symbol={symbol}&limit=100"))
    }

    /// Get open interest.
    ///
    /// For example:
    ///
    /// - <https://api.bitget.com/api/mix/v1/market/open-interest?symbol=BTCUSDT_UMCBL>
    pub fn fetch_open_interest(symbol: &str) -> Result<String> {
        gen_api!(format!("/api/mix/v1/market/open-interest?symbol={symbol}"))
    }
    
    /// Get ticker information for all symbols.
    ///
    /// For example: <https://api.bitget.com/api/mix/v1/market/tickers?productType=umcbl>
    ///
    /// Rate Limit: 20 requests per second (IP)
    ///
    /// Returns information including:
    /// - symbol: Symbol Id
    /// - last: Latest price
    /// - bestAsk/bestBid: Ask/Bid prices
    /// - high24h/low24h: Highest/Lowest price in 24 hours
    /// - baseVolume: Base currency trading volume
    /// - quoteVolume: Quote currency trading volume
    /// - fundingRate: Funding rate
    /// - holdingAmount: Holding amount
    pub async fn fetch_all_symbols() -> Result<Vec<Value>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        
        let usdt_futures_url = "https://api.bitget.com/api/mix/v1/market/contracts?productType=umcbl";
        let usdt_futures_response = client.get(usdt_futures_url).send().await?;
        let usdt_futures_json: Value = usdt_futures_response.json().await?;
        
        let mut contracts = Vec::new();
        if let Some(data) = usdt_futures_json["data"].as_array() {
            for contract in data {
                contracts.push(contract.clone());
            }
        }
       
        Ok(contracts)
    }
}
