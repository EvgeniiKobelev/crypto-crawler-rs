use async_trait::async_trait;
use crypto_ws_client::mexc::MexcUserDataStreamWSClient;
use crypto_ws_client::{
    BingxSpotWSClient, BingxSwapWSClient, MexcSpotWSClient, MexcSwapWSClient, WSClient,
};
use log::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc as async_mpsc;

use crate::config::ExchangeConfig;
use crate::exchange_type::ExchangeType;
use crate::traits::{SubscriptionManager, WebSocketClient};

/// Типы каналов подписки
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Orderbook,
    Trades,
    Ticker,
    Kline,
    AccountBalance,
    Orders,
    PrivateDeals,
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Orderbook => "orderbook",
            ChannelType::Trades => "trades",
            ChannelType::Ticker => "ticker",
            ChannelType::Kline => "kline",
            ChannelType::AccountBalance => "balance",
            ChannelType::Orders => "orders",
            ChannelType::PrivateDeals => "private_deals",
        }
    }
}

/// Конфигурация для подписки
#[derive(Debug, Clone)]
pub struct SubscriptionConfig {
    pub channel: ChannelType,
    pub symbol: String,
    pub interval: Option<String>, // для kline
}

/// Сообщение от WebSocket
#[derive(Debug, Clone)]
pub struct WsMessage {
    pub exchange: ExchangeType,
    pub channel: ChannelType,
    pub symbol: String,
    pub data: Value,
    pub timestamp: u64,
}

/// Состояние подключения
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// Менеджер подписок
#[derive(Debug, Default)]
pub struct SubscriptionManagerImpl {
    subscriptions: HashMap<ExchangeType, HashSet<(String, String)>>, // (channel, symbol)
}

impl SubscriptionManager for SubscriptionManagerImpl {
    fn add_subscription(&mut self, _channel: String, _symbol: String) {
        // Реализация добавления будет зависеть от конкретной биржи
        // Пока что сохраняем общую логику
    }

    fn remove_subscription(&mut self, _channel: String, _symbol: String) {
        // Аналогично для удаления
    }

    fn get_subscriptions(&self) -> Vec<(String, String)> {
        let mut all_subs = Vec::new();
        for subs in self.subscriptions.values() {
            for sub in subs {
                all_subs.push(sub.clone());
            }
        }
        all_subs
    }

    fn clear_subscriptions(&mut self) {
        self.subscriptions.clear();
    }
}

/// Потокобезопасный канал для получения сообщений
struct MessageChannel {
    sender: std::sync::mpsc::Sender<String>,
    receiver: Arc<Mutex<std::sync::mpsc::Receiver<String>>>,
}

impl MessageChannel {
    fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        debug!("MessageChannel::new: создан новый канал сообщений");
        Self { sender: tx, receiver: Arc::new(Mutex::new(rx)) }
    }

    fn try_recv(&self) -> Option<String> {
        if let Ok(receiver) = self.receiver.lock() {
            match receiver.try_recv() {
                Ok(msg) => {
                    debug!("MessageChannel::try_recv: получено сообщение из канала");
                    Some(msg)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    trace!("MessageChannel::try_recv: канал пуст");
                    None
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    warn!("MessageChannel::try_recv: канал отключен");
                    None
                }
            }
        } else {
            error!("MessageChannel::try_recv: ошибка блокировки mutex канала");
            None
        }
    }
}

/// Обёртка для различных WebSocket клиентов
pub enum WsClientWrapper {
    MexcSpot {
        client: Arc<MexcSpotWSClient>,
        message_channel: MessageChannel,
        is_running: Arc<Mutex<bool>>,
    },
    MexcUserDataStream {
        client: Arc<MexcUserDataStreamWSClient>,
        message_channel: MessageChannel,
        is_running: Arc<Mutex<bool>>,
    },
    MexcSwap {
        client: Arc<MexcSwapWSClient>,
        message_channel: MessageChannel,
        is_running: Arc<Mutex<bool>>,
    },
    BingxSpot {
        client: Arc<BingxSpotWSClient>,
        message_channel: MessageChannel,
        is_running: Arc<Mutex<bool>>,
    },
    BingxSwap {
        client: Arc<BingxSwapWSClient>,
        message_channel: MessageChannel,
        is_running: Arc<Mutex<bool>>,
    },
    Binance,     // TODO: добавить конкретные типы когда будут доступны
    Okx,         // TODO: из crypto-ws-client
    Bybit,       // TODO: из crypto-ws-client
    Huobi,       // TODO: из crypto-ws-client
    Kucoin,      // TODO: из crypto-ws-client
    Bitget,      // TODO: из crypto-ws-client
    Kraken,      // TODO: из crypto-ws-client
    Gate,        // TODO: из crypto-ws-client
    Placeholder, // Временный вариант для компиляции
}

impl WsClientWrapper {
    /// Создать новый WebSocket клиент для User Data Stream MEXC
    pub async fn new_mexc_user_data_stream(listen_key: &str) -> Result<Self, String> {
        info!("WsClientWrapper::new_mexc_user_data_stream: создание MEXC User Data Stream клиента");

        let channel = MessageChannel::new();
        debug!(
            "WsClientWrapper::new_mexc_user_data_stream: канал для MEXC User Data Stream создан"
        );

        let client = Arc::new(
            MexcUserDataStreamWSClient::new(listen_key, channel.sender.clone(), None).await,
        );
        debug!("WsClientWrapper::new_mexc_user_data_stream: MEXC User Data Stream WSClient создан");

        Ok(WsClientWrapper::MexcUserDataStream {
            client,
            message_channel: channel,
            is_running: Arc::new(Mutex::new(false)),
        })
    }

    /// Создать новый WebSocket клиент для указанной биржи
    pub async fn new(exchange_type: ExchangeType) -> Result<Self, String> {
        info!("WsClientWrapper::new: создание клиента для биржи {:?}", exchange_type);

        match exchange_type {
            ExchangeType::MexcSpot => {
                debug!("WsClientWrapper::new: создание MEXC Spot клиента");
                let channel = MessageChannel::new();
                debug!("WsClientWrapper::new: канал для MEXC Spot создан");

                let client = Arc::new(MexcSpotWSClient::new(channel.sender.clone(), None).await);
                debug!("WsClientWrapper::new: MEXC Spot WSClient создан");

                Ok(WsClientWrapper::MexcSpot {
                    client,
                    message_channel: channel,
                    is_running: Arc::new(Mutex::new(false)),
                })
            }
            ExchangeType::MexcSwap => {
                debug!("WsClientWrapper::new: создание MEXC Swap клиента");
                let channel = MessageChannel::new();
                debug!("WsClientWrapper::new: канал для MEXC Swap создан");

                let client = Arc::new(MexcSwapWSClient::new(channel.sender.clone(), None).await);
                debug!("WsClientWrapper::new: MEXC Swap WSClient создан");

                Ok(WsClientWrapper::MexcSwap {
                    client,
                    message_channel: channel,
                    is_running: Arc::new(Mutex::new(false)),
                })
            }
            ExchangeType::BingxSpot => {
                debug!("WsClientWrapper::new: создание BingX Spot клиента");
                let channel = MessageChannel::new();
                debug!("WsClientWrapper::new: канал для BingX Spot создан");

                let client = Arc::new(BingxSpotWSClient::new(channel.sender.clone(), None).await);
                debug!("WsClientWrapper::new: BingX Spot WSClient создан");

                Ok(WsClientWrapper::BingxSpot {
                    client,
                    message_channel: channel,
                    is_running: Arc::new(Mutex::new(false)),
                })
            }
            ExchangeType::BingxSwap => {
                debug!("WsClientWrapper::new: создание BingX Swap клиента");
                let channel = MessageChannel::new();
                debug!("WsClientWrapper::new: канал для BingX Swap создан");

                let client = Arc::new(BingxSwapWSClient::new(channel.sender.clone(), None).await);
                debug!("WsClientWrapper::new: BingX Swap WSClient создан");

                Ok(WsClientWrapper::BingxSwap {
                    client,
                    message_channel: channel,
                    is_running: Arc::new(Mutex::new(false)),
                })
            }
            _ => {
                warn!("WsClientWrapper::new: неподдерживаемая биржа {:?}", exchange_type);
                Err(format!("WebSocket клиент для биржи {:?} пока не реализован", exchange_type))
            }
        }
    }

    /// Запустить WebSocket клиент в фоновом режиме
    pub async fn start_background_task(&mut self) -> Result<(), String> {
        match self {
            WsClientWrapper::MexcSpot { client, is_running, .. } => {
                let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                if !*running {
                    let client_arc = Arc::clone(client);
                    tokio::spawn(async move {
                        info!("MEXC Spot WebSocket: запуск фоновой задачи");
                        client_arc.run().await;
                        info!("MEXC Spot WebSocket: фоновая задача завершена");
                    });
                    *running = true;
                    info!("MEXC Spot WebSocket клиент запущен в фоновом режиме");
                } else {
                    debug!("MEXC Spot WebSocket клиент уже запущен");
                }
                Ok(())
            }
            WsClientWrapper::MexcUserDataStream { client, is_running, .. } => {
                let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                if !*running {
                    // Запускаем MEXC User Data Stream клиент в фоновом режиме
                    let client_arc = Arc::clone(client);
                    tokio::spawn(async move {
                        info!("MEXC User Data Stream WebSocket: запуск фоновой задачи");
                        client_arc.run().await;
                        info!("MEXC User Data Stream WebSocket: фоновая задача завершена");
                    });
                    *running = true;
                    info!("MEXC User Data Stream WebSocket клиент запущен в фоновом режиме");
                } else {
                    debug!("MEXC User Data Stream WebSocket клиент уже запущен");
                }
                Ok(())
            }
            WsClientWrapper::MexcSwap { client, is_running, .. } => {
                let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                if !*running {
                    let client_arc = Arc::clone(client);
                    tokio::spawn(async move {
                        info!("MEXC Swap WebSocket: запуск фоновой задачи");
                        client_arc.run().await;
                        info!("MEXC Swap WebSocket: фоновая задача завершена");
                    });
                    *running = true;
                    info!("MEXC Swap WebSocket клиент запущен в фоновом режиме");
                } else {
                    debug!("MEXC Swap WebSocket клиент уже запущен");
                }
                Ok(())
            }
            WsClientWrapper::BingxSpot { client, is_running, .. } => {
                let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                if !*running {
                    let client_arc = Arc::clone(client);
                    tokio::spawn(async move {
                        info!("BingX Spot WebSocket: запуск фоновой задачи");
                        client_arc.run().await;
                        info!("BingX Spot WebSocket: фоновая задача завершена");
                    });
                    *running = true;
                    info!("BingX Spot WebSocket клиент запущен в фоновом режиме");
                } else {
                    debug!("BingX Spot WebSocket клиент уже запущен");
                }
                Ok(())
            }
            WsClientWrapper::BingxSwap { client, is_running, .. } => {
                let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                if !*running {
                    let client_arc = Arc::clone(client);
                    tokio::spawn(async move {
                        info!("BingX Swap WebSocket: запуск фоновой задачи");
                        client_arc.run().await;
                        info!("BingX Swap WebSocket: фоновая задача завершена");
                    });
                    *running = true;
                    info!("BingX Swap WebSocket клиент запущен в фоновом режиме");
                } else {
                    debug!("BingX Swap WebSocket клиент уже запущен");
                }
                Ok(())
            }
            WsClientWrapper::Placeholder => Ok(()),
            _ => Err("WebSocket клиенты пока не реализованы".to_string()),
        }
    }

    /// Получить следующее сообщение (неблокирующий вызов)
    pub fn try_recv_message(&mut self) -> Option<String> {
        let result = match self {
            WsClientWrapper::MexcSpot { message_channel, .. } => {
                trace!("try_recv_message: проверяем канал MEXC Spot");
                message_channel.try_recv()
            }
            WsClientWrapper::MexcUserDataStream { message_channel, .. } => {
                trace!("try_recv_message: проверяем канал MEXC User Data Stream");
                message_channel.try_recv()
            }
            WsClientWrapper::MexcSwap { message_channel, .. } => {
                trace!("try_recv_message: проверяем канал MEXC Swap");
                message_channel.try_recv()
            }
            WsClientWrapper::BingxSpot { message_channel, .. } => {
                trace!("try_recv_message: проверяем канал BingX Spot");
                message_channel.try_recv()
            }
            WsClientWrapper::BingxSwap { message_channel, .. } => {
                trace!("try_recv_message: проверяем канал BingX Swap");
                message_channel.try_recv()
            }
            _ => {
                trace!("try_recv_message: неподдерживаемый тип клиента");
                None
            }
        };

        if let Some(ref msg) = result {
            debug!("try_recv_message: получено сообщение длиной {} символов", msg.len());
        } else {
            trace!("try_recv_message: сообщений в канале нет");
        }

        result
    }
}

// Реализуем Send и Sync для WsClientWrapper
unsafe impl Send for WsClientWrapper {}
unsafe impl Sync for WsClientWrapper {}

#[async_trait]
impl WebSocketClient for WsClientWrapper {
    type Message = WsMessage;

    async fn connect(&mut self) -> Result<(), String> {
        // Запускаем фоновую задачу для обработки WebSocket соединения
        self.start_background_task().await
    }

    async fn disconnect(&mut self) -> Result<(), String> {
        match self {
            WsClientWrapper::MexcSpot { client, is_running, .. } => {
                let should_close = {
                    let running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running
                };
                if should_close {
                    client.close().await;
                    let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running = false;
                    info!("MEXC Spot WebSocket отключён");
                }
                Ok(())
            }
            WsClientWrapper::MexcUserDataStream { client, is_running, .. } => {
                let should_close = {
                    let running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running
                };
                if should_close {
                    client.close().await;
                    let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running = false;
                    info!("MEXC User Data Stream WebSocket отключён");
                }
                Ok(())
            }
            WsClientWrapper::MexcSwap { client, is_running, .. } => {
                let should_close = {
                    let running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running
                };
                if should_close {
                    client.close().await;
                    let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running = false;
                    info!("MEXC Swap WebSocket отключён");
                }
                Ok(())
            }
            WsClientWrapper::BingxSpot { client, is_running, .. } => {
                let should_close = {
                    let running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running
                };
                if should_close {
                    client.close().await;
                    let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running = false;
                    info!("BingX Spot WebSocket отключён");
                }
                Ok(())
            }
            WsClientWrapper::BingxSwap { client, is_running, .. } => {
                let should_close = {
                    let running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running
                };
                if should_close {
                    client.close().await;
                    let mut running = is_running.lock().map_err(|_| "Ошибка блокировки mutex")?;
                    *running = false;
                    info!("BingX Swap WebSocket отключён");
                }
                Ok(())
            }
            WsClientWrapper::Placeholder => Ok(()),
            _ => Err("WebSocket клиенты пока не реализованы".to_string()),
        }
    }

    async fn subscribe_orderbook(&mut self, symbol: &str) -> Result<(), String> {
        info!("subscribe_orderbook: начинаем подписку на orderbook для символа {}", symbol);

        match self {
            WsClientWrapper::MexcSpot { client, .. } => {
                info!("subscribe_orderbook: подписка на orderbook для MEXC Spot: {}", symbol);
                client.subscribe_orderbook(&[symbol.to_string()]).await;
                info!("subscribe_orderbook: подписка на MEXC Spot orderbook выполнена");
                Ok(())
            }
            WsClientWrapper::MexcUserDataStream { .. } => {
                warn!(
                    "subscribe_orderbook: MEXC User Data Stream предназначен только для приватных данных"
                );
                Err("MEXC User Data Stream не поддерживает публичные каналы как orderbook"
                    .to_string())
            }
            WsClientWrapper::MexcSwap { client, .. } => {
                info!("subscribe_orderbook: подписка на orderbook для MEXC Swap: {}", symbol);
                client.subscribe_orderbook(&[symbol.to_string()]).await;
                info!("subscribe_orderbook: подписка на MEXC Swap orderbook выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSpot { client, .. } => {
                info!("subscribe_orderbook: подписка на orderbook для BingX Spot: {}", symbol);
                client.subscribe_orderbook(&[symbol.to_string()]).await;
                info!("subscribe_orderbook: подписка на BingX Spot orderbook выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSwap { client, .. } => {
                info!("subscribe_orderbook: подписка на orderbook для BingX Swap: {}", symbol);
                client.subscribe_orderbook(&[symbol.to_string()]).await;
                info!("subscribe_orderbook: подписка на BingX Swap orderbook выполнена");
                Ok(())
            }
            WsClientWrapper::Placeholder => {
                debug!("subscribe_orderbook: пропуск placeholder клиента");
                Ok(())
            }
            _ => {
                warn!("subscribe_orderbook: неподдерживаемый тип клиента");
                Err("WebSocket клиенты пока не реализованы".to_string())
            }
        }
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> Result<(), String> {
        info!("subscribe_trades: начинаем подписку на trades для символа {}", symbol);

        match self {
            WsClientWrapper::MexcSpot { client, .. } => {
                info!("subscribe_trades: подписка на trades для MEXC Spot: {}", symbol);
                client.subscribe_trade(&[symbol.to_string()]).await;
                info!("subscribe_trades: подписка на MEXC Spot trades выполнена");
                Ok(())
            }
            WsClientWrapper::MexcUserDataStream { .. } => {
                warn!(
                    "subscribe_trades: MEXC User Data Stream предназначен только для приватных данных"
                );
                Err("MEXC User Data Stream не поддерживает публичные каналы как trades".to_string())
            }
            WsClientWrapper::MexcSwap { client, .. } => {
                info!("subscribe_trades: подписка на trades для MEXC Swap: {}", symbol);
                client.subscribe_trade(&[symbol.to_string()]).await;
                info!("subscribe_trades: подписка на MEXC Swap trades выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSpot { client, .. } => {
                info!("subscribe_trades: подписка на trades для BingX Spot: {}", symbol);
                client.subscribe_trade(&[symbol.to_string()]).await;
                info!("subscribe_trades: подписка на BingX Spot trades выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSwap { client, .. } => {
                info!("subscribe_trades: подписка на trades для BingX Swap: {}", symbol);
                client.subscribe_trade(&[symbol.to_string()]).await;
                info!("subscribe_trades: подписка на BingX Swap trades выполнена");
                Ok(())
            }
            WsClientWrapper::Placeholder => {
                debug!("subscribe_trades: пропуск placeholder клиента");
                Ok(())
            }
            _ => {
                warn!("subscribe_trades: неподдерживаемый тип клиента");
                Err("WebSocket клиенты пока не реализованы".to_string())
            }
        }
    }

    async fn subscribe_ticker(&mut self, symbol: &str) -> Result<(), String> {
        info!("subscribe_ticker: начинаем подписку на ticker для символа {}", symbol);

        match self {
            WsClientWrapper::MexcSpot { .. } => {
                warn!("subscribe_ticker: MEXC Spot не поддерживает ticker канал");
                Err("MEXC Spot не поддерживает ticker канал".to_string())
            }
            WsClientWrapper::MexcUserDataStream { .. } => {
                warn!(
                    "subscribe_ticker: MEXC User Data Stream предназначен только для приватных данных"
                );
                Err("MEXC User Data Stream не поддерживает публичные каналы как ticker".to_string())
            }
            WsClientWrapper::MexcSwap { client, .. } => {
                info!("subscribe_ticker: подписка на ticker для MEXC Swap: {}", symbol);
                client.subscribe_ticker(&[symbol.to_string()]).await;
                info!("subscribe_ticker: подписка на MEXC Swap ticker выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSpot { client, .. } => {
                info!("subscribe_ticker: подписка на ticker для BingX Spot: {}", symbol);
                client.subscribe_ticker(&[symbol.to_string()]).await;
                info!("subscribe_ticker: подписка на BingX Spot ticker выполнена");
                Ok(())
            }
            WsClientWrapper::BingxSwap { client, .. } => {
                info!("subscribe_ticker: подписка на ticker для BingX Swap: {}", symbol);
                client.subscribe_ticker(&[symbol.to_string()]).await;
                info!("subscribe_ticker: подписка на BingX Swap ticker выполнена");
                Ok(())
            }
            WsClientWrapper::Placeholder => {
                debug!("subscribe_ticker: пропуск placeholder клиента");
                Ok(())
            }
            _ => {
                warn!("subscribe_ticker: неподдерживаемый тип клиента");
                Err("WebSocket клиенты пока не реализованы".to_string())
            }
        }
    }

    async fn subscribe_account_balance(&mut self, _listen_key: Option<&str>) -> Result<(), String> {
        info!("subscribe_account_balance: начинаем подписку на баланс аккаунта");

        match self {
            WsClientWrapper::MexcSpot { .. } => {
                warn!("subscribe_account_balance: MEXC Spot не поддерживает приватные каналы");
                Err("MEXC Spot требует использования отдельного User Data Stream клиента для подписки на баланс аккаунта. Используйте WsClientWrapper::new_mexc_user_data_stream()".to_string())
            }
            WsClientWrapper::MexcUserDataStream { client, .. } => {
                info!(
                    "subscribe_account_balance: подписка на баланс аккаунта MEXC User Data Stream"
                );
                client.subscribe_account_balance().await;
                info!(
                    "subscribe_account_balance: подписка на MEXC User Data Stream баланс аккаунта выполнена"
                );
                Ok(())
            }
            WsClientWrapper::MexcSwap { .. } => {
                warn!(
                    "subscribe_account_balance: MEXC Swap требует приватный ключ для баланса аккаунта"
                );
                Err("MEXC Swap требует приватный ключ для подписки на баланс аккаунта".to_string())
            }
            WsClientWrapper::BingxSpot { .. } => {
                warn!(
                    "subscribe_account_balance: BingX Spot требует приватный ключ для баланса аккаунта"
                );
                Err("BingX Spot требует приватный ключ для подписки на баланс аккаунта".to_string())
            }
            WsClientWrapper::BingxSwap { .. } => {
                warn!(
                    "subscribe_account_balance: BingX Swap требует приватный ключ для баланса аккаунта"
                );
                Err("BingX Swap требует приватный ключ для подписки на баланс аккаунта".to_string())
            }
            WsClientWrapper::Placeholder => {
                debug!("subscribe_account_balance: пропуск placeholder клиента");
                Ok(())
            }
            _ => {
                warn!("subscribe_account_balance: неподдерживаемый тип клиента");
                Err("WebSocket клиенты пока не реализованы".to_string())
            }
        }
    }

    async fn subscribe_private_deals(&mut self, _listen_key: Option<&str>) -> Result<(), String> {
        info!("subscribe_private_deals: начинаем подписку на приватные сделки аккаунта");

        match self {
            WsClientWrapper::MexcSpot { .. } => {
                warn!("subscribe_private_deals: MEXC Spot не поддерживает приватные каналы");
                Err("MEXC Spot требует использования отдельного User Data Stream клиента для подписки на приватные сделки. Используйте WsClientWrapper::new_mexc_user_data_stream()".to_string())
            }
            WsClientWrapper::MexcUserDataStream { client, .. } => {
                info!(
                    "subscribe_private_deals: подписка на приватные сделки MEXC User Data Stream"
                );
                client.subscribe_private_deals().await;
                info!(
                    "subscribe_private_deals: подписка на MEXC User Data Stream приватные сделки выполнена"
                );
                Ok(())
            }
            WsClientWrapper::MexcSwap { .. } => {
                warn!(
                    "subscribe_private_deals: MEXC Swap не поддерживает приватные сделки через этот канал"
                );
                Err("MEXC Swap не поддерживает подписку на приватные сделки".to_string())
            }
            WsClientWrapper::BingxSpot { .. } => {
                warn!(
                    "subscribe_private_deals: BingX Spot не поддерживает приватные сделки через этот канал"
                );
                Err("BingX Spot не поддерживает подписку на приватные сделки".to_string())
            }
            WsClientWrapper::BingxSwap { .. } => {
                warn!(
                    "subscribe_private_deals: BingX Swap не поддерживает приватные сделки через этот канал"
                );
                Err("BingX Swap не поддерживает подписку на приватные сделки".to_string())
            }
            WsClientWrapper::Placeholder => {
                debug!("subscribe_private_deals: пропуск placeholder клиента");
                Ok(())
            }
            _ => {
                warn!("subscribe_private_deals: неподдерживаемый тип клиента");
                Err("WebSocket клиенты пока не реализованы".to_string())
            }
        }
    }

    async fn next_message(&mut self) -> Result<Option<Self::Message>, String> {
        debug!("WsClientWrapper::next_message: вызван метод получения следующего сообщения");

        // Проверяем наличие новых сообщений
        if let Some(raw_message) = self.try_recv_message() {
            debug!("WsClientWrapper::next_message: получено сырое сообщение: {}", raw_message);

            // Определяем тип биржи перед парсингом для избежания проблем с заимствованием
            let exchange_type = match self {
                WsClientWrapper::MexcSpot { .. } => {
                    debug!("WsClientWrapper::next_message: обрабатываем сообщение для MEXC Spot");
                    ExchangeType::MexcSpot
                }
                WsClientWrapper::MexcUserDataStream { .. } => {
                    info!(
                        "WsClientWrapper::next_message: обрабатываем сообщение для MEXC User Data Stream"
                    );
                    ExchangeType::MexcSpot // User Data Stream использует тот же тип парсинга, что и MexcSpot
                }
                WsClientWrapper::MexcSwap { .. } => {
                    debug!("WsClientWrapper::next_message: обрабатываем сообщение для MEXC Swap");
                    ExchangeType::MexcSwap
                }
                WsClientWrapper::BingxSpot { .. } => {
                    debug!("WsClientWrapper::next_message: обрабатываем сообщение для BingX Spot");
                    ExchangeType::BingxSpot
                }
                WsClientWrapper::BingxSwap { .. } => {
                    debug!("WsClientWrapper::next_message: обрабатываем сообщение для BingX Swap");
                    ExchangeType::BingxSwap
                }
                _ => {
                    warn!("WsClientWrapper::next_message: неподдерживаемый тип биржи");
                    return Err("Неподдерживаемый тип биржи".to_string());
                }
            };
            // Парсим сообщение
            match Self::parse_message_static(exchange_type, &raw_message) {
                Ok(ws_message) => Ok(Some(ws_message)),
                Err(e) => {
                    // Логируем только реальные ошибки, не служебные сообщения
                    if e == "Служебное сообщение" {
                        trace!(
                            "WsClientWrapper::next_message: пропущено служебное сообщение: {}",
                            raw_message
                        );
                    } else {
                        warn!(
                            "WsClientWrapper::next_message: ошибка парсинга сообщения: {} - {}",
                            e, raw_message
                        );
                    }
                    Ok(None)
                }
            }
        } else {
            trace!("WsClientWrapper::next_message: новых сообщений нет");
            Ok(None)
        }
    }

    fn is_connected(&self) -> bool {
        match self {
            WsClientWrapper::MexcSpot { is_running, .. } => {
                is_running.lock().map(|r| *r).unwrap_or(false)
            }
            WsClientWrapper::MexcUserDataStream { is_running, .. } => {
                is_running.lock().map(|r| *r).unwrap_or(false)
            }
            WsClientWrapper::MexcSwap { is_running, .. } => {
                is_running.lock().map(|r| *r).unwrap_or(false)
            }
            WsClientWrapper::BingxSpot { is_running, .. } => {
                is_running.lock().map(|r| *r).unwrap_or(false)
            }
            WsClientWrapper::BingxSwap { is_running, .. } => {
                is_running.lock().map(|r| *r).unwrap_or(false)
            }
            WsClientWrapper::Placeholder => false,
            _ => false,
        }
    }
}

impl WsClientWrapper {
    /// Парсит сырое WebSocket сообщение в структурированный формат
    fn parse_message_static(
        exchange_type: ExchangeType,
        raw_message: &str,
    ) -> Result<WsMessage, String> {
        debug!(
            "parse_message_static: начинаем парсинг сообщения длиной {} символов",
            raw_message.len()
        );

        // Парсим JSON
        let data: Value = serde_json::from_str(raw_message).map_err(|e| {
            error!(
                "parse_message_static: ошибка парсинга JSON: {} - сообщение: {}",
                e, raw_message
            );
            format!("Ошибка парсинга JSON: {}", e)
        })?;

        debug!(
            "parse_message_static: JSON успешно распаршен, содержит {} полей",
            data.as_object().map(|o| o.len()).unwrap_or(0)
        );

        // Проверяем, является ли это служебным сообщением
        if Self::is_service_message(&exchange_type, &data) {
            debug!("parse_message_static: пропускаем служебное сообщение: {}", raw_message);
            return Err("Служебное сообщение".to_string());
        }

        // Сначала проверяем, является ли это приватным сообщением
        debug!("parse_message_static: проверяем является ли сообщение приватным");
        if Self::is_private_message(&exchange_type, &data) {
            info!("parse_message_static: обнаружено приватное сообщение, начинаем парсинг");
            match Self::parse_private_message(exchange_type, &data, raw_message) {
                Ok(ws_message) => {
                    info!(
                        "parse_message_static: приватное сообщение успешно обработано: channel={:?}, symbol={}",
                        ws_message.channel, ws_message.symbol
                    );
                    return Ok(ws_message);
                }
                Err(e) => {
                    error!(
                        "parse_message_static: ошибка парсинга приватного сообщения: {} - сообщение: {}",
                        e, raw_message
                    );
                    return Err(e);
                }
            }
        }

        // Затем обрабатываем как обычное публичное сообщение
        debug!("parse_message_static: обрабатываем как публичное сообщение");
        let (channel_type, symbol) = Self::extract_channel_and_symbol(&exchange_type, &data)?;

        debug!(
            "parse_message_static: публичное сообщение обработано: channel={:?}, symbol={}",
            channel_type, symbol
        );

        Ok(WsMessage {
            exchange: exchange_type,
            channel: channel_type,
            symbol,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }

    /// Проверяет, является ли сообщение приватным
    fn is_private_message(exchange_type: &ExchangeType, data: &Value) -> bool {
        debug!("is_private_message: проверяем сообщение для биржи {:?}", exchange_type);

        match *exchange_type {
            ExchangeType::MexcSpot => {
                debug!("is_private_message: анализируем MEXC Spot сообщение");

                // Проверяем User Data Stream формат: {"channel": "spot@private.*.api.pb", ...}
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    debug!("is_private_message: найден канал 'channel': {}", channel_str);
                    if channel_str.contains("private") {
                        info!(
                            "is_private_message: обнаружен приватный канал в User Data Stream: {}",
                            channel_str
                        );
                        return true;
                    }
                } else {
                    debug!("is_private_message: поле 'channel' не найдено или не является строкой");
                }

                // Проверяем новый wrapper формат с privateDeals
                if data.get("privateDeals").is_some() {
                    info!("is_private_message: обнаружен privateDeals в данных");
                    return true;
                } else {
                    debug!("is_private_message: поле 'privateDeals' не найдено");
                }

                // Проверяем приватные поля в данных
                if data.get("privateAccount").is_some() {
                    info!("is_private_message: обнаружен privateAccount в данных");
                    return true;
                } else {
                    debug!("is_private_message: поле 'privateAccount' не найдено");
                }

                // Проверяем старый формат где приватные сделки путаются с публичными
                if let Some(d_data) = data.get("d") {
                    debug!("is_private_message: найдено поле 'd', проверяем содержимое");
                    if let Some(symbol_in_d) = d_data.get("symbol").and_then(|v| v.as_str()) {
                        debug!("is_private_message: найден symbol в d: {}", symbol_in_d);
                        if symbol_in_d.contains("private.deals") {
                            info!(
                                "is_private_message: обнаружена приватная сделка в поле d.symbol: {}",
                                symbol_in_d
                            );
                            return true;
                        }
                    } else {
                        debug!(
                            "is_private_message: поле 'd.symbol' не найдено или не является строкой"
                        );
                    }
                } else {
                    debug!("is_private_message: поле 'd' не найдено");
                }

                debug!("is_private_message: MEXC Spot сообщение не является приватным");
                false
            }
            ExchangeType::MexcSwap => {
                debug!("is_private_message: анализируем MEXC Swap сообщение");
                // Для MEXC Swap приватные сообщения обычно содержат "private" в канале
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    if channel_str.contains("private") || channel_str.contains("account") {
                        debug!(
                            "is_private_message: обнаружен приватный канал MEXC Swap: {}",
                            channel_str
                        );
                        return true;
                    }
                }
                false
            }
            ExchangeType::BingxSpot | ExchangeType::BingxSwap => {
                debug!("is_private_message: анализируем BingX сообщение");
                // Для BingX приватные сообщения обычно содержат "account" или "private" в dataType
                if let Some(data_type) = data.get("dataType").and_then(|v| v.as_str()) {
                    if data_type.contains("account") || data_type.contains("private") {
                        debug!(
                            "is_private_message: обнаружен приватный dataType BingX: {}",
                            data_type
                        );
                        return true;
                    }
                }
                false
            }
            _ => {
                debug!("is_private_message: неподдерживаемый тип биржи: {:?}", exchange_type);
                false
            }
        }
    }

    /// Парсит приватное сообщение
    fn parse_private_message(
        exchange_type: ExchangeType,
        data: &Value,
        _raw_message: &str,
    ) -> Result<WsMessage, String> {
        debug!("parse_private_message: начинаем парсинг для биржи {:?}", exchange_type);
        
        match exchange_type {
            ExchangeType::MexcSpot => {
                debug!("parse_private_message: обрабатываем MEXC Spot приватное сообщение");
                
                // Обрабатываем различные форматы приватных сообщений MEXC

                // 1. User Data Stream формат: {"channel": "spot@private.deals.v3.api.pb", "privateDeals": {...}}
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    debug!("parse_private_message: найден канал: {}", channel_str);
                    
                    if channel_str.contains("private.deals") {
                        info!("parse_private_message: обрабатываем User Data Stream private.deals");
                        
                        // Проверяем наличие privateDeals
                        if data.get("privateDeals").is_some() {
                            debug!("parse_private_message: найдено поле privateDeals");
                        } else {
                            warn!("parse_private_message: отсутствует поле privateDeals в private.deals канале");
                        }
                        
                        let symbol = data
                            .get("symbol")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                debug!("parse_private_message: символ не найден в корневом объекте, ищем в privateDeals");
                                // Пытаемся извлечь символ из данных privateDeals
                                data.get("privateDeals")
                                    .and_then(|pd| pd.get("feeCurrency"))
                                    .and_then(|v| v.as_str())
                                    .and_then(|fee_currency| {
                                        debug!("parse_private_message: найден feeCurrency: {}", fee_currency);
                                        if fee_currency == "MX" { 
                                            debug!("parse_private_message: feeCurrency=MX, используем MXUSDT");
                                            Some("MXUSDT") 
                                        } else { 
                                            debug!("parse_private_message: feeCurrency={}, не можем определить символ", fee_currency);
                                            None 
                                        }
                                    })
                            })
                            .unwrap_or("UNKNOWN")
                            .to_string();

                        info!(
                            "parse_private_message: обработана приватная сделка User Data Stream для символа: {}",
                            symbol
                        );

                        return Ok(WsMessage {
                            exchange: exchange_type,
                            channel: ChannelType::PrivateDeals,
                            symbol,
                            data: data.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        });
                    }

                    if channel_str.contains("private.account") || channel_str.contains("account") {
                        info!(
                            "parse_private_message: обработаны данные приватного аккаунта User Data Stream"
                        );

                        return Ok(WsMessage {
                            exchange: exchange_type,
                            channel: ChannelType::AccountBalance,
                            symbol: "ACCOUNT".to_string(),
                            data: data.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        });
                    }
                    
                    debug!("parse_private_message: канал {} не содержит private.deals или private.account", channel_str);
                } else {
                    debug!("parse_private_message: поле 'channel' не найдено, проверяем wrapper формат");
                }

                // 2. Новый wrapper формат с privateDeals
                if data.get("privateDeals").is_some() {
                    debug!("parse_private_message: обрабатываем wrapper формат с privateDeals");
                    
                    let symbol = data
                        .get("symbol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_string();

                    info!(
                        "parse_private_message: обработана приватная сделка wrapper формат для символа: {}",
                        symbol
                    );

                    return Ok(WsMessage {
                        exchange: exchange_type,
                        channel: ChannelType::PrivateDeals,
                        symbol,
                        data: data.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    });
                } else {
                    debug!("parse_private_message: поле 'privateDeals' не найдено, проверяем смешанный формат");
                }

                // 3. Старый смешанный формат где приватные сделки попадают в публичный канал
                if let Some(d_data) = data.get("d") {
                    debug!("parse_private_message: проверяем смешанный формат с полем 'd'");
                    
                    if let Some(symbol_in_d) = d_data.get("symbol").and_then(|v| v.as_str()) {
                        debug!("parse_private_message: найден symbol в d: {}", symbol_in_d);
                        
                        if symbol_in_d.contains("private.deals") {
                            debug!("parse_private_message: обрабатываем смешанный формат с private.deals");
                            
                            // Извлекаем реальный символ из поля quantity
                            let symbol = d_data
                                .get("quantity")
                                .and_then(|v| v.as_str())
                                .filter(|s| {
                                    !s.is_empty()
                                        && (s.contains("USDT")
                                            || s.contains("USDC")
                                            || s.contains("BTC"))
                                })
                                .unwrap_or("UNKNOWN")
                                .to_string();

                            warn!(
                                "parse_private_message: обработана приватная сделка из смешанного формата для символа: {}",
                                symbol
                            );

                            return Ok(WsMessage {
                                exchange: exchange_type,
                                channel: ChannelType::PrivateDeals,
                                symbol,
                                data: data.clone(),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64,
                            });
                        } else {
                            debug!("parse_private_message: symbol в d не содержит private.deals: {}", symbol_in_d);
                        }
                    } else {
                        debug!("parse_private_message: symbol в d не найден или не является строкой");
                    }
                } else {
                    debug!("parse_private_message: поле 'd' не найдено");
                }

                // 4. Другие приватные данные аккаунта
                if data.get("privateAccount").is_some() {
                    info!("parse_private_message: обработаны данные privateAccount");

                    return Ok(WsMessage {
                        exchange: exchange_type,
                        channel: ChannelType::AccountBalance,
                        symbol: "ACCOUNT".to_string(),
                        data: data.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    });
                } else {
                    debug!("parse_private_message: поле 'privateAccount' не найдено");
                }

                error!("parse_private_message: ни один формат приватного сообщения MEXC не подошел");
                Err("Неизвестный формат приватного сообщения MEXC".to_string())
            }
            ExchangeType::MexcSwap => {
                let channel_str = data.get("channel").and_then(|v| v.as_str()).unwrap_or("");

                if channel_str.contains("account") || channel_str.contains("private") {
                    info!(
                        "parse_private_message: обработано приватное сообщение MEXC Swap: {}",
                        channel_str
                    );

                    return Ok(WsMessage {
                        exchange: exchange_type,
                        channel: ChannelType::AccountBalance,
                        symbol: "ACCOUNT".to_string(),
                        data: data.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    });
                }

                Err("Неизвестный формат приватного сообщения MEXC Swap".to_string())
            }
            ExchangeType::BingxSpot | ExchangeType::BingxSwap => {
                let data_type = data.get("dataType").and_then(|v| v.as_str()).unwrap_or("");

                if data_type.contains("account") || data_type.contains("private") {
                    info!(
                        "parse_private_message: обработано приватное сообщение BingX: {}",
                        data_type
                    );

                    return Ok(WsMessage {
                        exchange: exchange_type,
                        channel: ChannelType::AccountBalance,
                        symbol: "ACCOUNT".to_string(),
                        data: data.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    });
                }

                Err("Неизвестный формат приватного сообщения BingX".to_string())
            }
            _ => Err("Неподдерживаемый тип биржи для приватных сообщений".to_string()),
        }
    }

    /// Извлекает тип канала и символ из WebSocket сообщения
    fn extract_channel_and_symbol(
        exchange_type: &ExchangeType,
        data: &Value,
    ) -> Result<(ChannelType, String), String> {
        match *exchange_type {
            ExchangeType::MexcSpot => {
                // Проверяем формат User Data Stream: {"channel": "spot@private.account.v3.api.pb", ...}
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    if channel_str.contains("private.account") || channel_str.contains("account") {
                        info!("Обнаружено сообщение User Data Stream: {}", channel_str);
                        return Ok((ChannelType::AccountBalance, "ACCOUNT".to_string()));
                    }
                    if channel_str.contains("private.deals") {
                        info!(
                            "Обнаружено сообщение приватных сделок User Data Stream: {}",
                            channel_str
                        );
                        // Извлекаем символ из данных сообщения privateDeals, если доступен
                        let symbol = data
                            .get("privateDeals")
                            .and_then(|pd| pd.get("symbol"))
                            .and_then(|v| v.as_str())
                            .or_else(|| data.get("symbol").and_then(|v| v.as_str()))
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        info!("Извлечен символ для приватных сделок: {}", symbol);
                        return Ok((ChannelType::PrivateDeals, symbol));
                    }
                }

                // MEXC Spot v3 API формат: {"c": "spot@public.deals.v3.api@BTCUSDT", "d": {...}, "t": timestamp}
                if let Some(channel_str) = data.get("c").and_then(|v| v.as_str()) {
                    if channel_str.contains("deals") {
                        // Проверяем, является ли это приватной сделкой по содержимому данных
                        let d_symbol = data
                            .get("d")
                            .and_then(|d| d.get("symbol"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if d_symbol.contains("private.deals") {
                            // Это приватная сделка - извлекаем реальный символ из поля quantity
                            info!(
                                "Обнаружена приватная сделка в формате MEXC v3: symbol в d = {}",
                                d_symbol
                            );

                            // Для приватных сделок символ попал в поле quantity из-за неправильного декодирования
                            let symbol = data
                                .get("d")
                                .and_then(|d| d.get("quantity"))
                                .and_then(|v| v.as_str())
                                .filter(|s| {
                                    !s.is_empty()
                                        && (s.contains("USDT")
                                            || s.contains("USDC")
                                            || s.contains("BTC"))
                                })
                                .unwrap_or("UNKNOWN")
                                .to_string();

                            info!("Извлечен символ для приватной сделки: {}", symbol);
                            return Ok((ChannelType::PrivateDeals, symbol));
                        } else {
                            // Обычные публичные сделки
                            // Проверяем, есть ли символ в конце канала
                            if let Ok(symbol) = Self::extract_mexc_symbol_from_channel(channel_str)
                            {
                                Ok((ChannelType::Trades, symbol))
                            } else {
                                // Канал без символа - пытаемся извлечь символ из данных
                                let symbol = data
                                    .get("d")
                                    .and_then(|d| d.get("symbol"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("UNKNOWN")
                                    .to_string();
                                debug!(
                                    "extract_channel_and_symbol: канал deals без символа, извлечен символ из данных: {}",
                                    symbol
                                );
                                Ok((ChannelType::Trades, symbol))
                            }
                        }
                    } else if channel_str.contains("depth")
                        || channel_str.contains("increase.depth")
                    {
                        let symbol = Self::extract_mexc_symbol_from_channel(channel_str)?;
                        Ok((ChannelType::Orderbook, symbol))
                    } else if channel_str.contains("kline") {
                        let symbol = Self::extract_mexc_symbol_from_channel(channel_str)?;
                        Ok((ChannelType::Kline, symbol))
                    } else if channel_str.contains("balance")
                        || channel_str.contains("account")
                        || channel_str.contains("executionReport")
                    {
                        // Для баланса аккаунта и приватных каналов символ не требуется
                        Ok((ChannelType::AccountBalance, "ACCOUNT".to_string()))
                    } else {
                        debug!(
                            "extract_channel_and_symbol: неизвестный канал MEXC Spot: {}",
                            channel_str
                        );
                        Err(format!("Неизвестный канал MEXC Spot: {}", channel_str))
                    }
                } else {
                    // Если нет ни "channel", ни "c", проверяем другие поля для User Data Stream
                    if data.get("privateAccount").is_some() || data.get("createTime").is_some() {
                        info!(
                            "Обнаружено сообщение User Data Stream по полям privateAccount/createTime"
                        );
                        return Ok((ChannelType::AccountBalance, "ACCOUNT".to_string()));
                    }
                    Err("Не найден канал в сообщении MEXC Spot".to_string())
                }
            }
            ExchangeType::MexcSwap => {
                // MEXC Swap формат: {"channel": "push.deal", "symbol": "BTC_USDT", "data": {...}, "ts": timestamp}
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    let channel_type = if channel_str.contains("deal") {
                        ChannelType::Trades
                    } else if channel_str.contains("depth") {
                        ChannelType::Orderbook
                    } else if channel_str.contains("ticker") {
                        ChannelType::Ticker
                    } else if channel_str.contains("balance") || channel_str.contains("account") {
                        ChannelType::AccountBalance
                    } else {
                        return Err(format!("Неизвестный канал MEXC Swap: {}", channel_str));
                    };

                    // Для баланса аккаунта символ может не потребоваться
                    let symbol = if matches!(channel_type, ChannelType::AccountBalance) {
                        "ACCOUNT".to_string()
                    } else {
                        data.get("symbol")
                            .and_then(|v| v.as_str())
                            .ok_or("Не найден символ в сообщении MEXC Swap")?
                            .to_string()
                    };

                    Ok((channel_type, symbol))
                } else {
                    Err("Не найден канал в сообщении MEXC Swap".to_string())
                }
            }
            ExchangeType::BingxSpot | ExchangeType::BingxSwap => {
                // BingX формат: {"dataType": "BTC-USDT@depth", "data": {...}, "ts": timestamp}
                if let Some(data_type) = data.get("dataType").and_then(|v| v.as_str()) {
                    let parts: Vec<&str> = data_type.split('@').collect();
                    if parts.len() != 2 {
                        return Err(format!("Неправильный формат dataType BingX: {}", data_type));
                    }

                    let channel_str = parts[1];

                    let channel_type = if channel_str == "trade" {
                        ChannelType::Trades
                    } else if channel_str == "depth" {
                        ChannelType::Orderbook
                    } else if channel_str == "ticker" {
                        ChannelType::Ticker
                    } else if channel_str.starts_with("kline") {
                        ChannelType::Kline
                    } else if channel_str == "balance" || channel_str == "account" {
                        ChannelType::AccountBalance
                    } else {
                        return Err(format!("Неизвестный канал BingX: {}", channel_str));
                    };

                    // Для баланса аккаунта символ может не потребоваться
                    let symbol = if matches!(channel_type, ChannelType::AccountBalance) {
                        "ACCOUNT".to_string()
                    } else {
                        parts[0].replace("-", "_") // BTC-USDT -> BTC_USDT
                    };

                    Ok((channel_type, symbol))
                } else {
                    Err("Не найден dataType в сообщении BingX".to_string())
                }
            }
            _ => Err("Неподдерживаемый тип клиента".to_string()),
        }
    }

    /// Извлекает символ из канала MEXC
    fn extract_mexc_symbol_from_channel(channel: &str) -> Result<String, String> {
        // Формат: "spot@public.deals.v3.api@BTCUSDT"
        let parts: Vec<&str> = channel.split('@').collect();
        if parts.len() < 3 {
            return Err(format!("Канал MEXC не содержит символа: {}", channel));
        }

        let symbol_part = parts.last().unwrap();

        // Проверяем, что последняя часть действительно символ, а не часть API пути
        if symbol_part.contains("api") || symbol_part.contains("pb") || symbol_part.contains("v3") {
            return Err(format!("Канал MEXC не содержит символа в конце: {}", channel));
        }

        // Конвертируем BTCUSDT обратно в BTC_USDT
        let symbol = if symbol_part.len() >= 6 && symbol_part.ends_with("USDT") {
            let base = &symbol_part[..symbol_part.len() - 4];
            format!("{}_USDT", base)
        } else if symbol_part.len() >= 7 && symbol_part.ends_with("USDC") {
            let base = &symbol_part[..symbol_part.len() - 4];
            format!("{}_USDC", base)
        } else {
            symbol_part.to_string()
        };

        Ok(symbol)
    }

    /// Проверяет, является ли сообщение служебным (не содержащим торговых данных)
    fn is_service_message(exchange_type: &ExchangeType, data: &Value) -> bool {
        match *exchange_type {
            ExchangeType::MexcSpot => {
                // Проверяем формат служебных сообщений MEXC Spot
                if let (Some(id), Some(code), Some(msg)) = (
                    data.get("id").and_then(|v| v.as_i64()),
                    data.get("code").and_then(|v| v.as_i64()),
                    data.get("msg").and_then(|v| v.as_str()),
                ) {
                    // Это ответ на подписку или служебное сообщение
                    debug!(
                        "is_service_message: обнаружено служебное сообщение MEXC Spot: id={}, code={}, msg={}",
                        id, code, msg
                    );
                    return true;
                }

                // Проверяем User Data Stream сообщения - они НЕ являются служебными
                if let Some(channel_str) = data.get("channel").and_then(|v| v.as_str()) {
                    if channel_str.contains("private") {
                        debug!(
                            "is_service_message: обнаружено User Data Stream приватное сообщение - НЕ служебное: {}",
                            channel_str
                        );
                        return false;
                    }
                }

                // Проверяем приватные поля - они НЕ являются служебными
                if data.get("privateDeals").is_some() 
                    || data.get("privateAccount").is_some() 
                    || data.get("createTime").is_some() {
                    debug!(
                        "is_service_message: обнаружено сообщение с приватными данными - НЕ служебное"
                    );
                    return false;
                }

                // Проверяем, есть ли поле "c" с данными канала (публичные сообщения)
                if data.get("c").is_none() && data.get("d").is_none() {
                    debug!("is_service_message: сообщение MEXC Spot не содержит данных канала");
                    return true;
                }

                false
            }
            ExchangeType::MexcSwap => {
                // Проверяем служебные сообщения MEXC Swap
                if let (Some(id), Some(code)) = (
                    data.get("id").and_then(|v| v.as_i64()),
                    data.get("code").and_then(|v| v.as_i64()),
                ) {
                    debug!(
                        "is_service_message: обнаружено служебное сообщение MEXC Swap: id={}, code={}",
                        id, code
                    );
                    return true;
                }

                // Проверяем наличие основных полей
                if data.get("channel").is_none() && data.get("data").is_none() {
                    debug!("is_service_message: сообщение MEXC Swap не содержит данных канала");
                    return true;
                }

                false
            }
            ExchangeType::BingxSpot | ExchangeType::BingxSwap => {
                // Проверяем служебные сообщения BingX
                if let Some(result) = data.get("result") {
                    if result.as_bool() == Some(true) || result.as_bool() == Some(false) {
                        debug!("is_service_message: обнаружено служебное сообщение BingX");
                        return true;
                    }
                }

                // Проверяем наличие основных полей
                if data.get("dataType").is_none() && data.get("data").is_none() {
                    debug!("is_service_message: сообщение BingX не содержит данных канала");
                    return true;
                }

                false
            }
            _ => false,
        }
    }
}

/// Фабрика для создания WebSocket клиентов
pub struct WsClientFactory;

impl WsClientFactory {
    pub async fn create_client(
        exchange_type: ExchangeType,
        _config: ExchangeConfig,
    ) -> Result<WsClientWrapper, String> {
        // Проверяем, поддерживает ли биржа WebSocket
        if !exchange_type.supports_websocket() {
            return Err(format!("WebSocket не поддерживается для биржи: {:?}", exchange_type));
        }

        match exchange_type {
            ExchangeType::MexcSpot => WsClientWrapper::new(ExchangeType::MexcSpot).await,
            ExchangeType::MexcSwap => WsClientWrapper::new(ExchangeType::MexcSwap).await,
            ExchangeType::BingxSpot => WsClientWrapper::new(ExchangeType::BingxSpot).await,
            ExchangeType::BingxSwap => WsClientWrapper::new(ExchangeType::BingxSwap).await,
            ExchangeType::BinanceSpot
            | ExchangeType::BinanceLinear
            | ExchangeType::BinanceInverse
            | ExchangeType::BinanceOption => Ok(WsClientWrapper::Binance),
            ExchangeType::OkxSpot => Ok(WsClientWrapper::Okx),
            ExchangeType::BybitLinear => Ok(WsClientWrapper::Bybit),
            ExchangeType::HuobiSpot => Ok(WsClientWrapper::Huobi),
            ExchangeType::KucoinSpot => Ok(WsClientWrapper::Kucoin),
            ExchangeType::BitgetSpot | ExchangeType::BitgetSwap => Ok(WsClientWrapper::Bitget),
            ExchangeType::KrakenSpot | ExchangeType::KrakenFutures => Ok(WsClientWrapper::Kraken),
            ExchangeType::GateSpot => Ok(WsClientWrapper::Gate),
            _ => Err(format!("WebSocket клиент для биржи {:?} пока не реализован", exchange_type)),
        }
    }
}

/// Основной унифицированный WebSocket клиент для всех криптовалютных бирж
pub struct CryptoWsClient {
    clients: HashMap<ExchangeType, WsClientWrapper>,
    message_sender: Option<async_mpsc::UnboundedSender<WsMessage>>,
    message_receiver: Option<async_mpsc::UnboundedReceiver<WsMessage>>,
    subscription_manager: SubscriptionManagerImpl,
    connection_states: HashMap<ExchangeType, ConnectionState>,
}

impl CryptoWsClient {
    /// Создание нового WebSocket клиента
    pub fn new() -> Self {
        let (sender, receiver) = async_mpsc::unbounded_channel();
        Self {
            clients: HashMap::new(),
            message_sender: Some(sender),
            message_receiver: Some(receiver),
            subscription_manager: SubscriptionManagerImpl::default(),
            connection_states: HashMap::new(),
        }
    }

    /// Добавить WebSocket клиент для биржи
    pub async fn add_exchange(
        &mut self,
        exchange_type: ExchangeType,
        config: ExchangeConfig,
    ) -> Result<(), String> {
        let client = WsClientFactory::create_client(exchange_type.clone(), config).await?;
        self.clients.insert(exchange_type.clone(), client);
        self.connection_states.insert(exchange_type, ConnectionState::Disconnected);
        Ok(())
    }

    /// Удалить WebSocket клиент
    pub async fn remove_exchange(&mut self, exchange_type: &ExchangeType) -> Result<(), String> {
        if let Some(mut client) = self.clients.remove(exchange_type) {
            let _ = client.disconnect().await;
        }
        self.connection_states.remove(exchange_type);
        Ok(())
    }

    /// Подключиться к биржам
    pub async fn connect_all(&mut self) -> Result<(), String> {
        for (exchange_type, client) in &mut self.clients {
            self.connection_states.insert(exchange_type.clone(), ConnectionState::Connecting);

            match client.connect().await {
                Ok(_) => {
                    self.connection_states
                        .insert(exchange_type.clone(), ConnectionState::Connected);
                }
                Err(e) => {
                    self.connection_states
                        .insert(exchange_type.clone(), ConnectionState::Error(e.clone()));
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Подключиться к конкретной бирже
    pub async fn connect_exchange(&mut self, exchange_type: &ExchangeType) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            self.connection_states.insert(exchange_type.clone(), ConnectionState::Connecting);

            match client.connect().await {
                Ok(_) => {
                    self.connection_states
                        .insert(exchange_type.clone(), ConnectionState::Connected);
                    Ok(())
                }
                Err(e) => {
                    self.connection_states
                        .insert(exchange_type.clone(), ConnectionState::Error(e.clone()));
                    Err(e)
                }
            }
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Отключиться от всех бирж
    pub async fn disconnect_all(&mut self) -> Result<(), String> {
        for (exchange_type, client) in &mut self.clients {
            let _ = client.disconnect().await;
            self.connection_states.insert(exchange_type.clone(), ConnectionState::Disconnected);
        }
        Ok(())
    }

    /// Подписаться на orderbook
    pub async fn subscribe_orderbook(
        &mut self,
        exchange_type: &ExchangeType,
        symbol: &str,
    ) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            client.subscribe_orderbook(symbol).await?;
            self.subscription_manager.add_subscription("orderbook".to_string(), symbol.to_string());
            Ok(())
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Подписаться на сделки
    pub async fn subscribe_trades(
        &mut self,
        exchange_type: &ExchangeType,
        symbol: &str,
    ) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            client.subscribe_trades(symbol).await?;
            self.subscription_manager.add_subscription("trades".to_string(), symbol.to_string());
            Ok(())
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Подписаться на тикеры
    pub async fn subscribe_ticker(
        &mut self,
        exchange_type: &ExchangeType,
        symbol: &str,
    ) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            client.subscribe_ticker(symbol).await?;
            self.subscription_manager.add_subscription("ticker".to_string(), symbol.to_string());
            Ok(())
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Подписаться на баланс аккаунта
    ///
    /// # Параметры
    /// * `exchange_type` - Тип биржи
    /// * `listen_key` - Опциональный ключ для приватных каналов (например, для MEXC)
    pub async fn subscribe_account_balance(
        &mut self,
        exchange_type: &ExchangeType,
        _listen_key: Option<&str>,
    ) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            client.subscribe_account_balance(_listen_key).await?;
            self.subscription_manager
                .add_subscription("balance".to_string(), "ACCOUNT".to_string());
            Ok(())
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Подписаться на приватные сделки аккаунта
    ///
    /// # Параметры
    /// * `exchange_type` - Тип биржи
    /// * `listen_key` - Опциональный ключ для приватных каналов (например, для MEXC)
    pub async fn subscribe_private_deals(
        &mut self,
        exchange_type: &ExchangeType,
        _listen_key: Option<&str>,
    ) -> Result<(), String> {
        if let Some(client) = self.clients.get_mut(exchange_type) {
            client.subscribe_private_deals(_listen_key).await?;
            self.subscription_manager
                .add_subscription("private_deals".to_string(), "ACCOUNT".to_string());
            Ok(())
        } else {
            Err(format!("Клиент для биржи {:?} не найден", exchange_type))
        }
    }

    /// Получить следующее сообщение из всех клиентов
    pub async fn next_message(&mut self) -> Result<Option<WsMessage>, String> {
        debug!("CryptoWsClient::next_message: запуск получения сообщений");

        // Проверяем сообщения от всех активных клиентов
        let connected_exchanges: Vec<_> = self
            .connection_states
            .iter()
            .filter(|(_, state)| matches!(state, ConnectionState::Connected))
            .map(|(exchange, _)| exchange.clone())
            .collect();

        debug!(
            "CryptoWsClient::next_message: найдено {} подключенных бирж: {:?}",
            connected_exchanges.len(),
            connected_exchanges
        );

        // Сначала проверяем неблокирующим способом наличие сообщений
        for exchange_type in &connected_exchanges {
            debug!(
                "CryptoWsClient::next_message: проверяем сообщения от биржи {:?}",
                exchange_type
            );

            if let Some(client) = self.clients.get_mut(exchange_type) {
                match client.next_message().await {
                    Ok(Some(message)) => {
                        // Возвращаем сообщение напрямую
                        return Ok(Some(message));
                    }
                    Ok(None) => {
                        trace!(
                            "CryptoWsClient::next_message: нет новых сообщений от биржи {:?}",
                            exchange_type
                        );
                    }
                    Err(e) => {
                        warn!(
                            "CryptoWsClient::next_message: ошибка получения сообщения от биржи {:?}: {}",
                            exchange_type, e
                        );
                    }
                }
            } else {
                warn!(
                    "CryptoWsClient::next_message: клиент для биржи {:?} не найден",
                    exchange_type
                );
            }
        }

        // Если нет сообщений от клиентов, возвращаем None
        trace!("CryptoWsClient::next_message: нет новых сообщений от всех бирж");
        Ok(None)
    }

    /// Получить следующее приватное сообщение (только PrivateDeals и AccountBalance)
    pub async fn next_private_message(&mut self) -> Result<Option<WsMessage>, String> {
        debug!("CryptoWsClient::next_private_message: запуск получения приватных сообщений");

        loop {
            match self.next_message().await? {
                Some(message) => {
                    // Проверяем, является ли сообщение приватным
                    if matches!(
                        message.channel,
                        ChannelType::PrivateDeals | ChannelType::AccountBalance
                    ) {
                        debug!(
                            "CryptoWsClient::next_private_message: получено приватное сообщение: {:?} для символа {}",
                            message.channel, message.symbol
                        );
                        return Ok(Some(message));
                    } else {
                        // Пропускаем публичные сообщения
                        trace!(
                            "CryptoWsClient::next_private_message: пропущено публичное сообщение: {:?}",
                            message.channel
                        );
                        continue;
                    }
                }
                None => {
                    // Нет новых сообщений
                    return Ok(None);
                }
            }
        }
    }

    /// Получить следующее публичное сообщение (исключая приватные)
    pub async fn next_public_message(&mut self) -> Result<Option<WsMessage>, String> {
        debug!("CryptoWsClient::next_public_message: запуск получения публичных сообщений");

        loop {
            match self.next_message().await? {
                Some(message) => {
                    // Проверяем, является ли сообщение публичным
                    if !matches!(
                        message.channel,
                        ChannelType::PrivateDeals | ChannelType::AccountBalance
                    ) {
                        debug!(
                            "CryptoWsClient::next_public_message: получено публичное сообщение: {:?} для символа {}",
                            message.channel, message.symbol
                        );
                        return Ok(Some(message));
                    } else {
                        // Пропускаем приватные сообщения
                        trace!(
                            "CryptoWsClient::next_public_message: пропущено приватное сообщение: {:?}",
                            message.channel
                        );
                        continue;
                    }
                }
                None => {
                    // Нет новых сообщений
                    return Ok(None);
                }
            }
        }
    }

    /// Проверить есть ли приватные сообщения определенного типа
    pub fn has_private_channel_subscriptions(&self, channel_type: ChannelType) -> bool {
        if !matches!(channel_type, ChannelType::PrivateDeals | ChannelType::AccountBalance) {
            return false;
        }

        let channel_str = channel_type.as_str();
        self.subscription_manager
            .get_subscriptions()
            .iter()
            .any(|(channel, _)| channel == channel_str)
    }

    /// Получить список активных приватных подписок
    pub fn get_private_subscriptions(&self) -> Vec<(String, String)> {
        self.subscription_manager
            .get_subscriptions()
            .into_iter()
            .filter(|(channel, _)| channel == "private_deals" || channel == "balance")
            .collect()
    }

    /// Получить состояние подключения для биржи
    pub fn get_connection_state(&self, exchange_type: &ExchangeType) -> Option<&ConnectionState> {
        self.connection_states.get(exchange_type)
    }

    /// Получить список подключённых бирж
    pub fn get_connected_exchanges(&self) -> Vec<ExchangeType> {
        self.connection_states
            .iter()
            .filter(|(_, state)| matches!(state, ConnectionState::Connected))
            .map(|(exchange, _)| exchange.clone())
            .collect()
    }

    /// Проверить, подключена ли конкретная биржа
    pub fn is_exchange_connected(&self, exchange_type: &ExchangeType) -> bool {
        matches!(self.connection_states.get(exchange_type), Some(ConnectionState::Connected))
    }

    /// Получить все активные подписки
    pub fn get_subscriptions(&self) -> Vec<(String, String)> {
        self.subscription_manager.get_subscriptions()
    }

    /// Получить количество настроенных WebSocket клиентов
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

impl Default for CryptoWsClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_private_deals_channel() {
        // Тестируем парсинг сообщения приватных сделок
        let raw_data = json!({
            "c": "spot@public.deals.v3.api",
            "d": {
                "price": "",
                "quantity": "CLOREUSDT",
                "symbol": "spot@private.deals.v3.api.pb",
                "takerOrderSide": 0,
                "time": 0
            },
            "t": 0
        });

        let result =
            WsClientWrapper::extract_channel_and_symbol(&ExchangeType::MexcSpot, &raw_data);

        match result {
            Ok((channel_type, symbol)) => {
                assert_eq!(channel_type, ChannelType::PrivateDeals);
                assert_eq!(symbol, "CLOREUSDT");
                println!("✅ Тест прошел: channel_type = {:?}, symbol = {}", channel_type, symbol);
            }
            Err(e) => {
                panic!("❌ Тест не прошел: {}", e);
            }
        }
    }

    #[test]
    fn test_extract_public_deals_channel() {
        // Тестируем парсинг обычных публичных сделок
        let raw_data = json!({
            "c": "spot@public.deals.v3.api@BTCUSDT",
            "d": {
                "symbol": "BTCUSDT",
                "price": "50000.00",
                "quantity": "0.1",
                "time": 1640995200000_i64,
                "takerOrderSide": 1
            },
            "t": 1640995200000_i64
        });

        let result =
            WsClientWrapper::extract_channel_and_symbol(&ExchangeType::MexcSpot, &raw_data);

        match result {
            Ok((channel_type, symbol)) => {
                assert_eq!(channel_type, ChannelType::Trades);
                assert_eq!(symbol, "BTC_USDT");
                println!("✅ Тест прошел: channel_type = {:?}, symbol = {}", channel_type, symbol);
            }
            Err(e) => {
                panic!("❌ Тест не прошел: {}", e);
            }
        }
    }

    #[test]
    fn test_extract_user_data_stream_private_deals() {
        // Тестируем парсинг User Data Stream приватных сделок
        let raw_data = json!({
            "channel": "spot@private.deals.v3.api.pb",
            "privateDeals": {
                "price": "1.2345",
                "quantity": "10.0",
                "amount": "12.345",
                "tradeType": 1,
                "isMaker": false,
                "isSelfTrade": false,
                "tradeId": "987654321",
                "clientOrderId": "my_order_123",
                "orderId": "order_789",
                "feeAmount": "0.0123",
                "feeCurrency": "USDT",
                "time": 1749495346968_i64
            },
            "sendTime": 1749495346968_i64
        });

        let result =
            WsClientWrapper::extract_channel_and_symbol(&ExchangeType::MexcSpot, &raw_data);

        match result {
            Ok((channel_type, symbol)) => {
                assert_eq!(channel_type, ChannelType::PrivateDeals);
                // Symbol будет "UNKNOWN" так как privateDeals не содержит symbol
                assert_eq!(symbol, "UNKNOWN");
                println!("✅ Тест прошел: channel_type = {:?}, symbol = {}", channel_type, symbol);
            }
            Err(e) => {
                panic!("❌ Тест не прошел: {}", e);
            }
        }
    }

    #[test]
    fn test_private_message_detection_and_parsing() {
        // Тестируем обнаружение и парсинг различных форматов приватных сообщений

        // 1. User Data Stream формат с privateDeals
        let user_data_stream_message = json!({
            "channel": "spot@private.deals.v3.api.pb",
            "symbol": "MXUSDT",
            "sendTime": 1736417034332_i64,
            "privateDeals": {
                "price": "3.6962",
                "quantity": "1",
                "amount": "3.6962",
                "tradeType": 2,
                "tradeId": "505979017439002624X1",
                "orderId": "C02__505979017439002624115",
                "feeAmount": "0.0003998377369698171",
                "feeCurrency": "MX",
                "time": 1736417034280_i64
            }
        });

        // Проверяем что это приватное сообщение
        assert!(WsClientWrapper::is_private_message(
            &ExchangeType::MexcSpot,
            &user_data_stream_message
        ));

        // Парсим приватное сообщение
        let result = WsClientWrapper::parse_private_message(
            ExchangeType::MexcSpot,
            &user_data_stream_message,
            &user_data_stream_message.to_string(),
        );

        assert!(result.is_ok());
        let ws_message = result.unwrap();
        assert_eq!(ws_message.channel, ChannelType::PrivateDeals);
        assert_eq!(ws_message.symbol, "MXUSDT");
        assert_eq!(ws_message.exchange, ExchangeType::MexcSpot);

        // 2. Старый смешанный формат где приватные сделки попадают в публичный канал
        let mixed_format_message = json!({
            "c": "spot@public.deals.v3.api",
            "d": {
                "price": "",
                "quantity": "CLOREUSDT",
                "symbol": "spot@private.deals.v3.api.pb",
                "takerOrderSide": 0,
                "time": 0
            },
            "t": 0
        });

        // Проверяем что это приватное сообщение
        assert!(WsClientWrapper::is_private_message(
            &ExchangeType::MexcSpot,
            &mixed_format_message
        ));

        // Парсим приватное сообщение
        let result = WsClientWrapper::parse_private_message(
            ExchangeType::MexcSpot,
            &mixed_format_message,
            &mixed_format_message.to_string(),
        );

        assert!(result.is_ok());
        let ws_message = result.unwrap();
        assert_eq!(ws_message.channel, ChannelType::PrivateDeals);
        assert_eq!(ws_message.symbol, "CLOREUSDT");
        assert_eq!(ws_message.exchange, ExchangeType::MexcSpot);

        // 3. Обычное публичное сообщение НЕ должно быть приватным
        let public_message = json!({
            "c": "spot@public.deals.v3.api@BTCUSDT",
            "d": {
                "symbol": "BTCUSDT",
                "price": "50000.00",
                "quantity": "0.1",
                "time": 1640995200000_i64,
                "takerOrderSide": 1
            },
            "t": 1640995200000_i64
        });

        // Проверяем что это НЕ приватное сообщение
        assert!(!WsClientWrapper::is_private_message(&ExchangeType::MexcSpot, &public_message));

        println!("✅ Тест обнаружения и парсинга приватных сообщений прошел успешно");
    }

    #[test]
    fn test_user_data_stream_private_message_parsing() {
        // Тестируем точный формат сообщения User Data Stream как у пользователя
        let user_data_stream_message = r#"{
            "channel": "spot@private.deals.v3.api.pb",
            "symbol": "MXUSDT",
            "sendTime": 1736417034332,
            "privateDeals": {
                "price": "3.6962",
                "quantity": "1",
                "amount": "3.6962",
                "tradeType": 2,
                "tradeId": "505979017439002624X1",
                "orderId": "C02__505979017439002624115",
                "feeAmount": "0.0003998377369698171",
                "feeCurrency": "MX",
                "time": 1736417034280
            }
        }"#;

        // Парсим JSON
        let data: Value = serde_json::from_str(user_data_stream_message).unwrap();

        // Проверяем что это распознается как приватное сообщение
        assert!(WsClientWrapper::is_private_message(&ExchangeType::MexcSpot, &data));

        // Парсим приватное сообщение
        let result = WsClientWrapper::parse_private_message(
            ExchangeType::MexcSpot,
            &data,
            user_data_stream_message,
        );

        assert!(result.is_ok());
        let ws_message = result.unwrap();
        assert_eq!(ws_message.channel, ChannelType::PrivateDeals);
        assert_eq!(ws_message.symbol, "MXUSDT");
        assert_eq!(ws_message.exchange, ExchangeType::MexcSpot);

        // Проверяем что данные сохранены правильно
        assert_eq!(
            ws_message.data.get("channel").unwrap().as_str().unwrap(),
            "spot@private.deals.v3.api.pb"
        );
        assert_eq!(ws_message.data.get("symbol").unwrap().as_str().unwrap(), "MXUSDT");

        let private_deals = ws_message.data.get("privateDeals").unwrap();
        assert_eq!(private_deals.get("price").unwrap().as_str().unwrap(), "3.6962");
        assert_eq!(private_deals.get("quantity").unwrap().as_str().unwrap(), "1");
        assert_eq!(private_deals.get("tradeType").unwrap().as_i64().unwrap(), 2);
        assert_eq!(private_deals.get("feeCurrency").unwrap().as_str().unwrap(), "MX");

        println!("✅ Тест парсинга User Data Stream приватных сообщений прошел успешно");
    }

    #[test]
    fn test_parse_message_static_with_user_data_stream() {
        // Тестируем полный pipeline parse_message_static с User Data Stream сообщением
        let user_data_stream_message = r#"{
            "channel": "spot@private.deals.v3.api.pb",
            "symbol": "MXUSDT",
            "sendTime": 1736417034332,
            "privateDeals": {
                "price": "3.6962",
                "quantity": "1",
                "amount": "3.6962",
                "tradeType": 2,
                "tradeId": "505979017439002624X1",
                "orderId": "C02__505979017439002624115",
                "feeAmount": "0.0003998377369698171",
                "feeCurrency": "MX",
                "time": 1736417034280
            }
        }"#;

        let result =
            WsClientWrapper::parse_message_static(ExchangeType::MexcSpot, user_data_stream_message);

        assert!(
            result.is_ok(),
            "parse_message_static должен успешно обработать User Data Stream сообщение"
        );

        let ws_message = result.unwrap();
        assert_eq!(ws_message.channel, ChannelType::PrivateDeals);
        assert_eq!(ws_message.symbol, "MXUSDT");
        assert_eq!(ws_message.exchange, ExchangeType::MexcSpot);

        println!("✅ Тест parse_message_static с User Data Stream прошел успешно");
        println!(
            "Результат: channel={:?}, symbol={}, exchange={:?}",
            ws_message.channel, ws_message.symbol, ws_message.exchange
        );
    }

    #[test]
    fn test_user_data_stream_not_service_message() {
        // Тестируем что User Data Stream сообщения НЕ классифицируются как служебные
        let user_data_stream_message = json!({
            "channel": "spot@private.deals.v3.api.pb",
            "privateDeals": {
                "amount": "1.19818703",
                "clientOrderId": "1897e831-fe41-4118-b5cf-fa2e3ec5",
                "feeAmount": "0.000599093515",
                "feeCurrency": "USDT",
                "isMaker": false,
                "isSelfTrade": false,
                "orderId": "C02__560846921632235520099",
                "price": "0.017561",
                "quantity": "68.23",
                "time": 1749498562026_i64,
                "tradeId": "560846921632235520X1",
                "tradeType": 1
            },
            "sendTime": 1749498562045_i64,
            "symbol": "CLOREUSDT"
        });

        // Проверяем что это НЕ служебное сообщение
        assert!(!WsClientWrapper::is_service_message(
            &ExchangeType::MexcSpot,
            &user_data_stream_message
        ), "User Data Stream сообщения с privateDeals НЕ должны быть служебными");

        // Проверяем что это приватное сообщение
        assert!(WsClientWrapper::is_private_message(
            &ExchangeType::MexcSpot,
            &user_data_stream_message
        ), "User Data Stream сообщения с privateDeals должны быть приватными");

        // Тестируем полный pipeline парсинга
        let result = WsClientWrapper::parse_message_static(
            ExchangeType::MexcSpot,
            &user_data_stream_message.to_string(),
        );

        assert!(
            result.is_ok(),
            "User Data Stream сообщение должно успешно парситься"
        );

        let ws_message = result.unwrap();
        assert_eq!(ws_message.channel, ChannelType::PrivateDeals);
        assert_eq!(ws_message.symbol, "CLOREUSDT");
        assert_eq!(ws_message.exchange, ExchangeType::MexcSpot);

        println!("✅ Тест классификации User Data Stream сообщений прошел успешно");
    }
}
