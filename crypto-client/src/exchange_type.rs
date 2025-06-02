/// Перечисление всех поддерживаемых типов клиентов криптобирж
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExchangeType {
    // Binance экосистема
    BinanceSpot,
    BinanceLinear,
    BinanceInverse,
    BinanceOption,

    // Другие крупные биржи
    OkxSpot,
    BybitLinear,
    HuobiSpot,
    KucoinSpot,
    MexcSpot,
    MexcSwap,

    // Spot биржи
    BingxSpot,
    BingxSwap,
    BitfinexSpot,
    BitgetSpot,
    BitgetSwap,
    BithumbSpot,
    BitmexSwap,
    BitstampSpot,
    BitzSpot,
    BitzSwap,
    CoinbaseProSpot,
    DeribitOptions,
    FtxSpot,
    GateSpot,
    KrakenSpot,
    KrakenFutures,
    ZbSpot,
    ZbSwap,
    ZbgSpot,
}

impl ExchangeType {
    /// Получить строковое представление биржи
    pub fn as_str(&self) -> &'static str {
        match self {
            ExchangeType::BinanceSpot => "binance_spot",
            ExchangeType::BinanceLinear => "binance_linear",
            ExchangeType::BinanceInverse => "binance_inverse",
            ExchangeType::BinanceOption => "binance_option",
            ExchangeType::OkxSpot => "okx",
            ExchangeType::BybitLinear => "bybit",
            ExchangeType::HuobiSpot => "huobi_spot",
            ExchangeType::KucoinSpot => "kucoin_spot",
            ExchangeType::MexcSpot => "mexc_spot",
            ExchangeType::MexcSwap => "mexc_swap",
            ExchangeType::BingxSpot => "bingx_spot",
            ExchangeType::BingxSwap => "bingx_swap",
            ExchangeType::BitfinexSpot => "bitfinex",
            ExchangeType::BitgetSpot => "bitget_spot",
            ExchangeType::BitgetSwap => "bitget_swap",
            ExchangeType::BithumbSpot => "bithumb",
            ExchangeType::BitmexSwap => "bitmex",
            ExchangeType::BitstampSpot => "bitstamp",
            ExchangeType::BitzSpot => "bitz_spot",
            ExchangeType::BitzSwap => "bitz_swap",
            ExchangeType::CoinbaseProSpot => "coinbase_pro",
            ExchangeType::DeribitOptions => "deribit",
            ExchangeType::FtxSpot => "ftx",
            ExchangeType::GateSpot => "gate",
            ExchangeType::KrakenSpot => "kraken_spot",
            ExchangeType::KrakenFutures => "kraken_futures",
            ExchangeType::ZbSpot => "zb_spot",
            ExchangeType::ZbSwap => "zb_swap",
            ExchangeType::ZbgSpot => "zbg",
        }
    }

    /// Проверить, поддерживает ли биржа WebSocket соединения
    pub fn supports_websocket(&self) -> bool {
        matches!(
            self,
            ExchangeType::BinanceSpot
                | ExchangeType::BinanceLinear
                | ExchangeType::BinanceInverse
                | ExchangeType::BinanceOption
                | ExchangeType::OkxSpot
                | ExchangeType::BybitLinear
                | ExchangeType::HuobiSpot
                | ExchangeType::KucoinSpot
                | ExchangeType::MexcSpot
                | ExchangeType::MexcSwap
                | ExchangeType::BingxSpot
                | ExchangeType::BingxSwap
                | ExchangeType::BitgetSpot
                | ExchangeType::BitgetSwap
                | ExchangeType::KrakenSpot
                | ExchangeType::KrakenFutures
        )
    }

    /// Получить все доступные типы бирж
    pub fn all() -> Vec<ExchangeType> {
        vec![
            ExchangeType::BinanceSpot,
            ExchangeType::BinanceLinear,
            ExchangeType::BinanceInverse,
            ExchangeType::BinanceOption,
            ExchangeType::OkxSpot,
            ExchangeType::BybitLinear,
            ExchangeType::HuobiSpot,
            ExchangeType::KucoinSpot,
            ExchangeType::MexcSpot,
            ExchangeType::MexcSwap,
            ExchangeType::BingxSpot,
            ExchangeType::BingxSwap,
            ExchangeType::BitfinexSpot,
            ExchangeType::BitgetSpot,
            ExchangeType::BitgetSwap,
            ExchangeType::BithumbSpot,
            ExchangeType::BitmexSwap,
            ExchangeType::BitstampSpot,
            ExchangeType::BitzSpot,
            ExchangeType::BitzSwap,
            ExchangeType::CoinbaseProSpot,
            ExchangeType::DeribitOptions,
            ExchangeType::FtxSpot,
            ExchangeType::GateSpot,
            ExchangeType::KrakenSpot,
            ExchangeType::KrakenFutures,
            ExchangeType::ZbSpot,
            ExchangeType::ZbSwap,
            ExchangeType::ZbgSpot,
        ]
    }
}
