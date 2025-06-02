use async_trait::async_trait;
use crypto_rest_client::*;
use std::collections::HashMap;

use crate::config::ExchangeConfig;
use crate::exchange_type::ExchangeType;
use crate::traits::ExchangeClient;

/// Обёртка для различных REST клиентов
pub enum RestClientWrapper {
    BinanceSpot(BinanceSpotRestClient),
    BinanceLinear(BinanceLinearRestClient),
    BinanceInverse(BinanceInverseRestClient),
    BinanceOption(BinanceOptionRestClient),
    Okx(OkxRestClient),
    Bybit(BybitRestClient),
    HuobiSpot(HuobiSpotRestClient),
    KucoinSpot(KuCoinSpotRestClient),
    MexcSpot(MexcSpotRestClient),
    MexcSwap(MexcSwapRestClient),
    BingxSpot(BingxSpotRestClient),
    BingxSwap(BingxSwapRestClient),
    Bitfinex(BitfinexRestClient),
    BitgetSpot(BitgetSpotRestClient),
    BitgetSwap(BitgetSwapRestClient),
    Bithumb(BithumbRestClient),
    Bitmex(BitmexRestClient),
    Bitstamp(BitstampRestClient),
    BitzSpot(BitzSpotRestClient),
    BitzSwap(BitzSwapRestClient),
    CoinbasePro(CoinbaseProRestClient),
    Deribit(DeribitRestClient),
    Ftx(FtxRestClient),
    KrakenSpot(KrakenSpotRestClient),
    KrakenFutures(KrakenFuturesRestClient),
    ZbSpot(ZbSpotRestClient),
    ZbSwap(ZbSwapRestClient),
}

#[async_trait]
impl ExchangeClient for RestClientWrapper {
    fn exchange_type(&self) -> ExchangeType {
        match self {
            RestClientWrapper::BinanceSpot(_) => ExchangeType::BinanceSpot,
            RestClientWrapper::BinanceLinear(_) => ExchangeType::BinanceLinear,
            RestClientWrapper::BinanceInverse(_) => ExchangeType::BinanceInverse,
            RestClientWrapper::BinanceOption(_) => ExchangeType::BinanceOption,
            RestClientWrapper::Okx(_) => ExchangeType::OkxSpot,
            RestClientWrapper::Bybit(_) => ExchangeType::BybitLinear,
            RestClientWrapper::HuobiSpot(_) => ExchangeType::HuobiSpot,
            RestClientWrapper::KucoinSpot(_) => ExchangeType::KucoinSpot,
            RestClientWrapper::MexcSpot(_) => ExchangeType::MexcSpot,
            RestClientWrapper::MexcSwap(_) => ExchangeType::MexcSwap,
            RestClientWrapper::BingxSpot(_) => ExchangeType::BingxSpot,
            RestClientWrapper::BingxSwap(_) => ExchangeType::BingxSwap,
            RestClientWrapper::Bitfinex(_) => ExchangeType::BitfinexSpot,
            RestClientWrapper::BitgetSpot(_) => ExchangeType::BitgetSpot,
            RestClientWrapper::BitgetSwap(_) => ExchangeType::BitgetSwap,
            RestClientWrapper::Bithumb(_) => ExchangeType::BithumbSpot,
            RestClientWrapper::Bitmex(_) => ExchangeType::BitmexSwap,
            RestClientWrapper::Bitstamp(_) => ExchangeType::BitstampSpot,
            RestClientWrapper::BitzSpot(_) => ExchangeType::BitzSpot,
            RestClientWrapper::BitzSwap(_) => ExchangeType::BitzSwap,
            RestClientWrapper::CoinbasePro(_) => ExchangeType::CoinbaseProSpot,
            RestClientWrapper::Deribit(_) => ExchangeType::DeribitOptions,
            RestClientWrapper::Ftx(_) => ExchangeType::FtxSpot,
            RestClientWrapper::KrakenSpot(_) => ExchangeType::KrakenSpot,
            RestClientWrapper::KrakenFutures(_) => ExchangeType::KrakenFutures,
            RestClientWrapper::ZbSpot(_) => ExchangeType::ZbSpot,
            RestClientWrapper::ZbSwap(_) => ExchangeType::ZbSwap,
        }
    }

    async fn fetch_l2_snapshot(&self, symbol: &str) -> Result<String, String> {
        let result = match self {
            RestClientWrapper::BinanceSpot(_) => {
                BinanceSpotRestClient::fetch_l2_snapshot(symbol).await
            }
            RestClientWrapper::BinanceLinear(_) => {
                BinanceLinearRestClient::fetch_l2_snapshot(symbol).await
            }
            RestClientWrapper::BinanceInverse(_) => {
                BinanceInverseRestClient::fetch_l2_snapshot(symbol).await
            }
            RestClientWrapper::BinanceOption(_) => {
                BinanceOptionRestClient::fetch_l2_snapshot(symbol).await
            }
            RestClientWrapper::Okx(_) => OkxRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Bybit(_) => BybitRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::HuobiSpot(_) => HuobiSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::KucoinSpot(_) => KuCoinSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::MexcSpot(_) => MexcSpotRestClient::fetch_l2_snapshot(symbol).await,
            RestClientWrapper::MexcSwap(_) => MexcSwapRestClient::fetch_l2_snapshot(symbol).await,
            RestClientWrapper::BingxSpot(_) => BingxSpotRestClient::fetch_l2_snapshot(symbol).await,
            RestClientWrapper::BingxSwap(_) => BingxSwapRestClient::fetch_l2_snapshot(symbol).await,
            RestClientWrapper::Bitfinex(_) => BitfinexRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::BitgetSpot(_) => BitgetSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::BitgetSwap(_) => BitgetSwapRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Bithumb(_) => BithumbRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Bitmex(_) => BitmexRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Bitstamp(_) => BitstampRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::BitzSpot(_) => BitzSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::BitzSwap(_) => BitzSwapRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::CoinbasePro(_) => CoinbaseProRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Deribit(_) => DeribitRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::Ftx(_) => FtxRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::KrakenSpot(_) => KrakenSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::KrakenFutures(_) => {
                KrakenFuturesRestClient::fetch_l2_snapshot(symbol)
            }
            RestClientWrapper::ZbSpot(_) => ZbSpotRestClient::fetch_l2_snapshot(symbol),
            RestClientWrapper::ZbSwap(_) => ZbSwapRestClient::fetch_l2_snapshot(symbol),
        };

        result.map_err(|e| e.to_string())
    }

    async fn get_balance(&self, asset: &str) -> Result<String, String> {
        let result = match self {
            RestClientWrapper::BinanceSpot(client) => client.get_account_balance(asset).await,
            RestClientWrapper::MexcSpot(client) => client.get_account_balance(asset).await,
            RestClientWrapper::BingxSpot(client) => client.get_account_balance(Some(asset)).await,
            _ => {
                return Err("Получение баланса пока не поддерживается для этой биржи".to_string());
            }
        };

        result.map_err(|e| e.to_string())
    }

    async fn create_limit_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<String, String> {
        let result = match self {
            RestClientWrapper::MexcSpot(client) => {
                client.create_order(symbol, side, quantity, price).await
            }
            RestClientWrapper::BingxSpot(client) => {
                client.create_order(symbol, side, quantity, Some(price), "LIMIT").await
            }
            _ => {
                return Err(
                    "Создание лимитных ордеров пока не поддерживается для этой биржи".to_string()
                );
            }
        };

        result.map_err(|e| e.to_string())
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<String, String> {
        let result = match self {
            RestClientWrapper::MexcSpot(client) => client.cancel_order(symbol, order_id).await,
            RestClientWrapper::BingxSpot(client) => client.cancel_order(symbol, order_id).await,
            _ => {
                return Err("Отмена ордеров пока не поддерживается для этой биржи".to_string());
            }
        };

        result.map_err(|e| e.to_string())
    }

    async fn get_listen_key(&self) -> Result<String, String> {
        let result = match self {
            RestClientWrapper::MexcSpot(client) => client.get_listen_key().await,
            RestClientWrapper::MexcSwap(client) => client.get_listen_key().await,
            _ => {
                return Err("get_listen_key поддерживается только для MEXC".to_string());
            }
        };

        result.map_err(|e| e.to_string())
    }
}

/// Фабрика для создания клиентов бирж
pub struct ExchangeClientFactory;

impl ExchangeClientFactory {
    pub fn create_client(
        exchange_type: ExchangeType,
        config: ExchangeConfig,
    ) -> Result<RestClientWrapper, String> {
        let client = match exchange_type {
            ExchangeType::BinanceSpot => RestClientWrapper::BinanceSpot(
                BinanceSpotRestClient::new(config.api_key, config.secret_key, config.proxy),
            ),
            ExchangeType::BinanceLinear => RestClientWrapper::BinanceLinear(
                BinanceLinearRestClient::new(config.api_key, config.secret_key, config.proxy),
            ),
            ExchangeType::BinanceInverse => RestClientWrapper::BinanceInverse(
                BinanceInverseRestClient::new(config.api_key, config.secret_key),
            ),
            ExchangeType::BinanceOption => RestClientWrapper::BinanceOption(
                BinanceOptionRestClient::new(config.api_key, config.secret_key),
            ),
            ExchangeType::OkxSpot => {
                RestClientWrapper::Okx(OkxRestClient::new(config.api_key, config.secret_key))
            }
            ExchangeType::BybitLinear => RestClientWrapper::Bybit(BybitRestClient::new(
                config.api_key,
                config.secret_key,
                config.proxy,
            )),
            ExchangeType::HuobiSpot => RestClientWrapper::HuobiSpot(HuobiSpotRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::KucoinSpot => RestClientWrapper::KucoinSpot(KuCoinSpotRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::MexcSpot => RestClientWrapper::MexcSpot(MexcSpotRestClient::new(
                config.api_key,
                config.secret_key,
                config.proxy,
            )),
            ExchangeType::MexcSwap => RestClientWrapper::MexcSwap(MexcSwapRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BingxSpot => RestClientWrapper::BingxSpot(BingxSpotRestClient::new(
                config.api_key,
                config.secret_key,
                config.proxy,
            )),
            ExchangeType::BingxSwap => RestClientWrapper::BingxSwap(BingxSwapRestClient::new(
                config.api_key,
                config.secret_key,
                config.proxy,
            )),
            ExchangeType::BitfinexSpot => RestClientWrapper::Bitfinex(BitfinexRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BitgetSpot => RestClientWrapper::BitgetSpot(BitgetSpotRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BitgetSwap => RestClientWrapper::BitgetSwap(BitgetSwapRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BithumbSpot => RestClientWrapper::Bithumb(BithumbRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BitmexSwap => {
                RestClientWrapper::Bitmex(BitmexRestClient::new(config.api_key, config.secret_key))
            }
            ExchangeType::BitstampSpot => RestClientWrapper::Bitstamp(BitstampRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BitzSpot => RestClientWrapper::BitzSpot(BitzSpotRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::BitzSwap => RestClientWrapper::BitzSwap(BitzSwapRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::CoinbaseProSpot => RestClientWrapper::CoinbasePro(
                CoinbaseProRestClient::new(config.api_key, config.secret_key),
            ),
            ExchangeType::DeribitOptions => RestClientWrapper::Deribit(DeribitRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::FtxSpot => {
                RestClientWrapper::Ftx(FtxRestClient::new(config.api_key, config.secret_key))
            }
            ExchangeType::GateSpot => {
                return Err("Gate exchange пока не поддерживается".to_string());
            }
            ExchangeType::KrakenSpot => RestClientWrapper::KrakenSpot(KrakenSpotRestClient::new(
                config.api_key,
                config.secret_key,
            )),
            ExchangeType::KrakenFutures => RestClientWrapper::KrakenFutures(
                KrakenFuturesRestClient::new(config.api_key, config.secret_key),
            ),
            ExchangeType::ZbSpot => {
                RestClientWrapper::ZbSpot(ZbSpotRestClient::new(config.api_key, config.secret_key))
            }
            ExchangeType::ZbSwap => {
                RestClientWrapper::ZbSwap(ZbSwapRestClient::new(config.api_key, config.secret_key))
            }
            ExchangeType::ZbgSpot => {
                return Err("ZBG exchange пока не поддерживается".to_string());
            }
        };

        Ok(client)
    }
}

/// Основной унифицированный REST клиент для всех криптовалютных бирж
pub struct CryptoRestClient {
    clients: HashMap<ExchangeType, RestClientWrapper>,
}

impl CryptoRestClient {
    /// Создание нового пустого клиента
    pub fn new() -> Self {
        Self { clients: HashMap::new() }
    }

    /// Добавить биржу в клиент
    pub fn add_exchange(
        &mut self,
        exchange_type: ExchangeType,
        config: ExchangeConfig,
    ) -> Result<(), String> {
        let client = ExchangeClientFactory::create_client(exchange_type.clone(), config)?;
        self.clients.insert(exchange_type, client);
        Ok(())
    }

    /// Удалить биржу из клиента
    pub fn remove_exchange(&mut self, exchange_type: &ExchangeType) -> bool {
        self.clients.remove(exchange_type).is_some()
    }

    /// Получить снимок orderbook уровня 2 для указанной биржи
    pub async fn fetch_l2_snapshot(
        &self,
        exchange_type: &ExchangeType,
        symbol: &str,
    ) -> Result<String, String> {
        match self.clients.get(exchange_type) {
            Some(client) => client.fetch_l2_snapshot(symbol).await,
            None => Err(format!("Клиент для биржи {:?} не настроен", exchange_type)),
        }
    }

    /// Получить баланс для указанной биржи
    pub async fn get_balance(
        &self,
        exchange_type: &ExchangeType,
        asset: &str,
    ) -> Result<String, String> {
        match self.clients.get(exchange_type) {
            Some(client) => client.get_balance(asset).await,
            None => Err(format!("Клиент для биржи {:?} не настроен", exchange_type)),
        }
    }

    /// Создать лимитный ордер для указанной биржи
    pub async fn create_limit_order(
        &self,
        exchange_type: &ExchangeType,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<String, String> {
        match self.clients.get(exchange_type) {
            Some(client) => client.create_limit_order(symbol, side, quantity, price).await,
            None => Err(format!("Клиент для биржи {:?} не настроен", exchange_type)),
        }
    }

    /// Отменить ордер для указанной биржи
    pub async fn cancel_order(
        &self,
        exchange_type: &ExchangeType,
        symbol: &str,
        order_id: &str,
    ) -> Result<String, String> {
        match self.clients.get(exchange_type) {
            Some(client) => client.cancel_order(symbol, order_id).await,
            None => Err(format!("Клиент для биржи {:?} не настроен", exchange_type)),
        }
    }

    /// Получить список доступных бирж
    pub fn get_available_exchanges(&self) -> Vec<ExchangeType> {
        self.clients.keys().cloned().collect()
    }

    /// Проверить, доступна ли биржа
    pub fn is_exchange_available(&self, exchange_type: &ExchangeType) -> bool {
        self.clients.contains_key(exchange_type)
    }

    /// Получить снимки orderbook для всех настроенных бирж
    pub async fn fetch_all_l2_snapshots(
        &self,
        symbol: &str,
    ) -> HashMap<ExchangeType, Result<String, String>> {
        let mut results = HashMap::new();
        for (exchange_type, client) in &self.clients {
            let result = client.fetch_l2_snapshot(symbol).await;
            results.insert(exchange_type.clone(), result);
        }
        results
    }

    /// Получить количество настроенных бирж
    pub fn exchange_count(&self) -> usize {
        self.clients.len()
    }

    /// Получить listen_key для WebSocket приватных данных
    pub async fn get_listen_key(&self, exchange_type: &ExchangeType) -> Result<String, String> {
        match self.clients.get(exchange_type) {
            Some(client) => client.get_listen_key().await,
            None => Err(format!("Клиент для биржи {:?} не настроен", exchange_type)),
        }
    }
}

impl Default for CryptoRestClient {
    fn default() -> Self {
        Self::new()
    }
}
