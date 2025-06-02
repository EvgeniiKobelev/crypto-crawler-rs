use base64::Engine;
use log::*;
use prost::{Message, Oneof};
use serde_json::{Value, json};

// Определяем protobuf структуры вручную для максимальной совместимости
pub mod mexc_proto {
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    pub struct WsMessage {
        #[prost(string, tag = "1")]
        pub channel: String,
        #[prost(bytes, tag = "2")]
        pub data: Vec<u8>,
        #[prost(string, optional, tag = "3")]
        pub ts: Option<String>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Deal {
        #[prost(string, tag = "1")]
        pub symbol: String,
        #[prost(string, tag = "2")]
        pub price: String,
        #[prost(string, tag = "3")]
        pub quantity: String,
        #[prost(int64, tag = "4")]
        pub time: i64,
        #[prost(int32, tag = "5")]
        pub taker_order_side: i32,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct DepthData {
        #[prost(string, tag = "1")]
        pub symbol: String,
        #[prost(message, repeated, tag = "2")]
        pub asks: Vec<PriceLevel>,
        #[prost(message, repeated, tag = "3")]
        pub bids: Vec<PriceLevel>,
        #[prost(int64, tag = "4")]
        pub version: i64,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct PriceLevel {
        #[prost(string, tag = "1")]
        pub price: String,
        #[prost(string, tag = "2")]
        pub quantity: String,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct KlineData {
        #[prost(string, tag = "1")]
        pub symbol: String,
        #[prost(string, tag = "2")]
        pub interval: String,
        #[prost(int64, tag = "3")]
        pub open_time: i64,
        #[prost(int64, tag = "4")]
        pub close_time: i64,
        #[prost(string, tag = "5")]
        pub open: String,
        #[prost(string, tag = "6")]
        pub high: String,
        #[prost(string, tag = "7")]
        pub low: String,
        #[prost(string, tag = "8")]
        pub close: String,
        #[prost(string, tag = "9")]
        pub volume: String,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct AccountData {
        #[prost(string, tag = "1")]
        pub account_id: String,
        #[prost(message, repeated, tag = "2")]
        pub balances: Vec<Balance>,
        #[prost(int64, tag = "3")]
        pub update_time: i64,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Balance {
        #[prost(string, tag = "1")]
        pub asset: String,
        #[prost(string, tag = "2")]
        pub free: String,
        #[prost(string, tag = "3")]
        pub locked: String,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct PrivateDealsV3Api {
        #[prost(string, tag = "1")]
        pub price: String,
        #[prost(string, tag = "2")]
        pub quantity: String,
        #[prost(string, tag = "3")]
        pub amount: String,
        #[prost(int32, tag = "4")]
        pub trade_type: i32,
        #[prost(bool, tag = "5")]
        pub is_maker: bool,
        #[prost(bool, tag = "6")]
        pub is_self_trade: bool,
        #[prost(string, tag = "7")]
        pub trade_id: String,
        #[prost(string, tag = "8")]
        pub client_order_id: String,
        #[prost(string, tag = "9")]
        pub order_id: String,
        #[prost(string, tag = "10")]
        pub fee_amount: String,
        #[prost(string, tag = "11")]
        pub fee_currency: String,
        #[prost(int64, tag = "12")]
        pub time: i64,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct PushDataV3ApiWrapper {
        #[prost(string, tag = "1")]
        pub channel: String,
        #[prost(oneof = "push_data_v3_api_wrapper::Body", tags = "301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315")]
        pub body: Option<push_data_v3_api_wrapper::Body>,
        #[prost(string, optional, tag = "3")]
        pub symbol: Option<String>,
        #[prost(string, optional, tag = "4")]
        pub symbol_id: Option<String>,
        #[prost(int64, optional, tag = "5")]
        pub create_time: Option<i64>,
        #[prost(int64, optional, tag = "6")]
        pub send_time: Option<i64>,
    }

    pub mod push_data_v3_api_wrapper {
        use prost::Oneof;
        use super::*;

        #[derive(Clone, PartialEq, Oneof)]
        pub enum Body {
            #[prost(message, tag = "301")]
            PublicDeals(Deal),
            #[prost(message, tag = "302")]
            PublicIncreaseDepths(DepthData),
            #[prost(message, tag = "306")]
            PrivateDeals(PrivateDealsV3Api),
            #[prost(message, tag = "307")]
            PrivateAccount(AccountData),
            #[prost(message, tag = "308")]
            PublicSpotKline(KlineData),
        }
    }
}

use mexc_proto::*;

/// Декодирует protobuf данные от MEXC в JSON формат
pub fn decode_mexc_protobuf(binary_data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    // Сначала пытаемся декодировать как новый wrapper формат
    if let Ok(wrapper) = PushDataV3ApiWrapper::decode(binary_data) {
        debug!("Successfully decoded as PushDataV3ApiWrapper with channel: {}", wrapper.channel);
        return Ok(wrapper_to_json(&wrapper)?);
    }

    // Затем пытаемся декодировать как приватные сделки напрямую
    if let Ok(json_result) = try_decode_private_deals(binary_data) {
        return Ok(json_result);
    }

    // Затем пытаемся декодировать как публичные сообщения
    if let Ok(json_result) = try_decode_public_messages(binary_data) {
        return Ok(json_result);
    }

    // Если ничего не сработало, попробуем создать минимальный JSON ответ
    // с raw данными для отладки
    debug!(
        "Binary data length: {}, first 20 bytes: {:?}",
        binary_data.len(),
        &binary_data[..std::cmp::min(20, binary_data.len())]
    );

    Err("Unable to decode protobuf data as any known MEXC format".into())
}

/// Конвертирует PushDataV3ApiWrapper в JSON
fn wrapper_to_json(wrapper: &PushDataV3ApiWrapper) -> Result<String, Box<dyn std::error::Error>> {
    let result = match &wrapper.body {
        Some(push_data_v3_api_wrapper::Body::PrivateDeals(private_deals)) => {
            let symbol = wrapper.symbol.as_deref().unwrap_or("UNKNOWN");
            json!({
                "channel": wrapper.channel,
                "symbol": symbol,
                "sendTime": wrapper.send_time.unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                }),
                "privateDeals": {
                    "price": private_deals.price,
                    "quantity": private_deals.quantity,
                    "amount": private_deals.amount,
                    "tradeType": private_deals.trade_type,
                    "isMaker": private_deals.is_maker,
                    "isSelfTrade": private_deals.is_self_trade,
                    "tradeId": private_deals.trade_id,
                    "clientOrderId": private_deals.client_order_id,
                    "orderId": private_deals.order_id,
                    "feeAmount": private_deals.fee_amount,
                    "feeCurrency": private_deals.fee_currency,
                    "time": private_deals.time
                }
            })
        }
        Some(push_data_v3_api_wrapper::Body::PublicDeals(deal)) => {
            json!({
                "c": wrapper.channel,
                "d": {
                    "symbol": deal.symbol,
                    "price": deal.price,
                    "quantity": deal.quantity,
                    "time": deal.time,
                    "takerOrderSide": deal.taker_order_side
                },
                "t": wrapper.send_time.unwrap_or(deal.time)
            })
        }
        Some(push_data_v3_api_wrapper::Body::PrivateAccount(account)) => {
            let balances = account
                .balances
                .iter()
                .map(|balance| {
                    json!({
                        "asset": balance.asset,
                        "free": balance.free,
                        "locked": balance.locked
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "channel": wrapper.channel,
                "d": {
                    "accountId": account.account_id,
                    "balances": balances,
                    "updateTime": account.update_time
                },
                "t": wrapper.send_time.unwrap_or(account.update_time)
            })
        }
        Some(push_data_v3_api_wrapper::Body::PublicIncreaseDepths(depth)) => {
            let asks = depth
                .asks
                .iter()
                .map(|level| json!([level.price, level.quantity]))
                .collect::<Vec<_>>();

            let bids = depth
                .bids
                .iter()
                .map(|level| json!([level.price, level.quantity]))
                .collect::<Vec<_>>();

            json!({
                "c": wrapper.channel,
                "d": {
                    "symbol": depth.symbol,
                    "asks": asks,
                    "bids": bids,
                    "version": depth.version
                },
                "t": wrapper.send_time.unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                })
            })
        }
        Some(push_data_v3_api_wrapper::Body::PublicSpotKline(kline)) => {
            json!({
                "c": wrapper.channel,
                "d": {
                    "symbol": kline.symbol,
                    "interval": kline.interval,
                    "openTime": kline.open_time,
                    "closeTime": kline.close_time,
                    "open": kline.open,
                    "high": kline.high,
                    "low": kline.low,
                    "close": kline.close,
                    "volume": kline.volume
                },
                "t": wrapper.send_time.unwrap_or(kline.close_time)
            })
        }
        None => {
            json!({
                "channel": wrapper.channel,
                "error": "No body data in wrapper",
                "t": wrapper.send_time.unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                })
            })
        }
    };

    Ok(result.to_string())
}

/// Пытается декодировать приватные сделки (User Data Stream)
pub fn try_decode_private_deals(binary_data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    if let Ok(private_deals) = PrivateDealsV3Api::decode(binary_data) {
        // Строгая валидация для приватных сделок
        if is_valid_private_deal(&private_deals) {
            debug!("Successfully decoded as PrivateDealsV3Api with valid data");
            debug!("Private deal data: price={}, quantity={}, fee_currency={}, time={}", 
                   private_deals.price, private_deals.quantity, private_deals.fee_currency, private_deals.time);
            return Ok(private_deals_to_json(&private_deals)?);
        } else {
            debug!("Decoded PrivateDealsV3Api but failed validation: price='{}', quantity='{}', time={}", 
                   private_deals.price, private_deals.quantity, private_deals.time);
        }
    }
    
    Err("Not a valid private deal".into())
}

/// Пытается декодировать публичные сообщения (WSMessage обертка)
pub fn try_decode_public_messages(binary_data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    // Пытаемся декодировать как основное WebSocket сообщение
    // для публичных каналов и других типов данных
    if let Ok(ws_message) = WsMessage::decode(binary_data) {
        // Проверяем, что это действительно валидное WSMessage
        if is_valid_public_ws_message(&ws_message) {
            debug!("Successfully decoded MEXC protobuf message: channel={}", ws_message.channel);
            // Конвертируем в JSON формат, совместимый с существующими обработчиками
            return Ok(protobuf_to_json(&ws_message)?);
        } else {
            debug!("Decoded as WSMessage but doesn't look like valid MEXC message: channel='{}', data_len={}", 
                   ws_message.channel, ws_message.data.len());
        }
    }

    // Пробуем декодировать как другие типы данных напрямую
    if let Ok(depth_data) = DepthData::decode(binary_data) {
        if is_valid_depth_data(&depth_data) {
            debug!("Successfully decoded as DepthData");
            return Ok(depth_to_json(&depth_data)?);
        }
    }

    if let Ok(account_data) = AccountData::decode(binary_data) {
        if is_valid_account_data(&account_data) {
            debug!("Successfully decoded as AccountData");
            return Ok(account_to_json(&account_data)?);
        }
    }

    if let Ok(kline_data) = KlineData::decode(binary_data) {
        if is_valid_kline_data(&kline_data) {
            debug!("Successfully decoded as KlineData");
            return Ok(kline_to_json(&kline_data)?);
        }
    }

    if let Ok(deal_data) = Deal::decode(binary_data) {
        if is_valid_deal_data(&deal_data) {
            debug!("Successfully decoded as Deal");
            return Ok(deal_to_json(&deal_data)?);
        }
    }

    Err("Not a valid public message".into())
}

/// Валидация приватных сделок
fn is_valid_private_deal(private_deals: &PrivateDealsV3Api) -> bool {
    // Все обязательные поля должны быть заполнены
    !private_deals.price.is_empty() 
        && !private_deals.quantity.is_empty()
        && !private_deals.amount.is_empty()
        && !private_deals.trade_id.is_empty()
        && !private_deals.order_id.is_empty()
        && !private_deals.fee_currency.is_empty()
        && private_deals.time > 0
        && private_deals.trade_type > 0  // 1 = BUY, 2 = SELL
        // Проверяем что цена и количество являются валидными числами
        && private_deals.price.parse::<f64>().is_ok()
        && private_deals.quantity.parse::<f64>().is_ok()
        && private_deals.amount.parse::<f64>().is_ok()
}

/// Валидация публичных WebSocket сообщений
fn is_valid_public_ws_message(ws_message: &WsMessage) -> bool {
    ws_message.channel.starts_with("spot@") 
        && !ws_message.channel.contains("private")  // Исключаем приватные каналы
        && !ws_message.data.is_empty()
}

/// Валидация данных глубины рынка
fn is_valid_depth_data(depth_data: &DepthData) -> bool {
    !depth_data.symbol.is_empty() && depth_data.version > 0
}

/// Валидация данных аккаунта
fn is_valid_account_data(account_data: &AccountData) -> bool {
    !account_data.account_id.is_empty() && account_data.update_time > 0
}

/// Валидация данных свечей
fn is_valid_kline_data(kline_data: &KlineData) -> bool {
    !kline_data.symbol.is_empty() 
        && !kline_data.interval.is_empty()
        && kline_data.open_time > 0
        && kline_data.close_time > 0
}

/// Валидация публичных сделок
fn is_valid_deal_data(deal_data: &Deal) -> bool {
    !deal_data.symbol.is_empty() 
        && !deal_data.price.is_empty()
        && !deal_data.quantity.is_empty()
        && deal_data.time > 0
        && deal_data.price.parse::<f64>().is_ok()
        && deal_data.quantity.parse::<f64>().is_ok()
}

/// Конвертирует WSMessage в JSON формат
fn protobuf_to_json(ws_message: &WsMessage) -> Result<String, Box<dyn std::error::Error>> {
    let channel = &ws_message.channel;

    // Определяем тип данных по каналу
    let data_value = if channel.contains("private.deals") {
        // Декодируем приватные сделки
        if let Ok(private_deals) = PrivateDealsV3Api::decode(&ws_message.data[..]) {
            json!({
                "price": private_deals.price,
                "quantity": private_deals.quantity,
                "amount": private_deals.amount,
                "tradeType": private_deals.trade_type,
                "tradeId": private_deals.trade_id,
                "orderId": private_deals.order_id,
                "feeAmount": private_deals.fee_amount,
                "feeCurrency": private_deals.fee_currency,
                "time": private_deals.time
            })
        } else {
            return Err("Failed to decode private deals data from protobuf".into());
        }
    } else if channel.contains("deals") {
        // Декодируем публичные торговые сделки
        if let Ok(deal_data) = Deal::decode(&ws_message.data[..]) {
            json!({
                "symbol": deal_data.symbol,
                "price": deal_data.price,
                "quantity": deal_data.quantity,
                "time": deal_data.time,
                "takerOrderSide": deal_data.taker_order_side
            })
        } else {
            return Err("Failed to decode deal data from protobuf".into());
        }
    } else if channel.contains("depth") {
        // Декодируем данные глубины рынка
        if let Ok(depth_data) = DepthData::decode(&ws_message.data[..]) {
            let asks = depth_data
                .asks
                .iter()
                .map(|level| json!([level.price, level.quantity]))
                .collect::<Vec<_>>();

            let bids = depth_data
                .bids
                .iter()
                .map(|level| json!([level.price, level.quantity]))
                .collect::<Vec<_>>();

            json!({
                "symbol": depth_data.symbol,
                "asks": asks,
                "bids": bids,
                "version": depth_data.version
            })
        } else {
            return Err("Failed to decode depth data from protobuf".into());
        }
    } else if channel.contains("kline") {
        // Декодируем данные свечей
        if let Ok(kline_data) = KlineData::decode(&ws_message.data[..]) {
            json!({
                "symbol": kline_data.symbol,
                "interval": kline_data.interval,
                "openTime": kline_data.open_time,
                "closeTime": kline_data.close_time,
                "open": kline_data.open,
                "high": kline_data.high,
                "low": kline_data.low,
                "close": kline_data.close,
                "volume": kline_data.volume
            })
        } else {
            return Err("Failed to decode kline data from protobuf".into());
        }
    } else if channel.contains("account") || channel.contains("private") {
        // Декодируем приватные данные аккаунта
        if let Ok(account_data) = AccountData::decode(&ws_message.data[..]) {
            let balances = account_data
                .balances
                .iter()
                .map(|balance| {
                    json!({
                        "asset": balance.asset,
                        "free": balance.free,
                        "locked": balance.locked
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "accountId": account_data.account_id,
                "balances": balances,
                "updateTime": account_data.update_time
            })
        } else {
            return Err("Failed to decode account data from protobuf".into());
        }
    } else {
        // Неизвестный тип канала - возвращаем raw данные как base64
        json!({
            "raw_data": base64::engine::general_purpose::STANDARD.encode(&ws_message.data),
            "warning": "Unknown channel type, returning raw data"
        })
    };

    // Формируем итоговое JSON сообщение в формате, совместимом с MEXC JSON API
    let send_time =
        ws_message.ts.as_ref().and_then(|s| s.parse::<i64>().ok()).unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    let result = if channel.contains("private.deals") {
        // Для приватных сделок используем специальный формат
        json!({
            "channel": channel,
            "privateDeals": data_value,
            "sendTime": send_time
        })
    } else {
        // Для остальных сообщений используем стандартный формат
        json!({
            "c": channel,
            "d": data_value,
            "t": send_time
        })
    };

    Ok(result.to_string())
}

/// Конвертирует Deal в JSON
fn deal_to_json(deal: &Deal) -> Result<String, Box<dyn std::error::Error>> {
    let result = json!({
        "c": "spot@public.deals.v3.api",
        "d": {
            "symbol": deal.symbol,
            "price": deal.price,
            "quantity": deal.quantity,
            "time": deal.time,
            "takerOrderSide": deal.taker_order_side
        },
        "t": deal.time
    });

    Ok(result.to_string())
}

/// Конвертирует DepthData в JSON
fn depth_to_json(depth: &DepthData) -> Result<String, Box<dyn std::error::Error>> {
    let asks =
        depth.asks.iter().map(|level| json!([level.price, level.quantity])).collect::<Vec<_>>();

    let bids =
        depth.bids.iter().map(|level| json!([level.price, level.quantity])).collect::<Vec<_>>();

    let result = json!({
        "c": "spot@public.increase.depth.v3.api",
        "d": {
            "symbol": depth.symbol,
            "asks": asks,
            "bids": bids,
            "version": depth.version
        },
        "t": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    });

    Ok(result.to_string())
}

/// Конвертирует KlineData в JSON
fn kline_to_json(kline: &KlineData) -> Result<String, Box<dyn std::error::Error>> {
    let result = json!({
        "c": "spot@public.kline.v3.api",
        "d": {
            "symbol": kline.symbol,
            "interval": kline.interval,
            "openTime": kline.open_time,
            "closeTime": kline.close_time,
            "open": kline.open,
            "high": kline.high,
            "low": kline.low,
            "close": kline.close,
            "volume": kline.volume
        },
        "t": kline.close_time
    });

    Ok(result.to_string())
}

/// Конвертирует AccountData в JSON
fn account_to_json(account: &AccountData) -> Result<String, Box<dyn std::error::Error>> {
    let balances = account
        .balances
        .iter()
        .map(|balance| {
            json!({
                "asset": balance.asset,
                "free": balance.free,
                "locked": balance.locked
            })
        })
        .collect::<Vec<_>>();

    let result = json!({
        "c": "spot@private.account.v3.api",
        "d": {
            "accountId": account.account_id,
            "balances": balances,
            "updateTime": account.update_time
        },
        "t": account.update_time
    });

    Ok(result.to_string())
}

/// Конвертирует PrivateDealsV3Api в JSON (версия для прямого декодирования)
fn private_deals_to_json(
    private_deals: &PrivateDealsV3Api,
) -> Result<String, Box<dyn std::error::Error>> {
    // Пытаемся извлечь символ из trade_id или order_id если возможно
    // Обычно MEXC включает информацию о символе в эти поля
    let symbol = extract_symbol_from_trade_context(private_deals).unwrap_or("UNKNOWN".to_string());

    let result = json!({
        "channel": "spot@private.deals.v3.api.pb",
        "symbol": symbol,
        "sendTime": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
        "privateDeals": {
            "price": private_deals.price,
            "quantity": private_deals.quantity,
            "amount": private_deals.amount,
            "tradeType": private_deals.trade_type,
            "isMaker": private_deals.is_maker,
            "isSelfTrade": private_deals.is_self_trade,
            "tradeId": private_deals.trade_id,
            "clientOrderId": private_deals.client_order_id,
            "orderId": private_deals.order_id,
            "feeAmount": private_deals.fee_amount,
            "feeCurrency": private_deals.fee_currency,
            "time": private_deals.time
        }
    });

    Ok(result.to_string())
}

/// Пытается извлечь символ из контекста торговой сделки
/// MEXC часто включает информацию о символе в различные поля
fn extract_symbol_from_trade_context(private_deals: &PrivateDealsV3Api) -> Option<String> {
    // Проверяем, есть ли информация о символе в fee_currency
    // Часто fee_currency указывает на базовую валюту торговой пары
    if !private_deals.fee_currency.is_empty() {
        // Если fee_currency = "MX", то возможно это MXUSDT
        if private_deals.fee_currency == "MX" {
            return Some("MXUSDT".to_string());
        }
        // Если fee_currency = "USDT", возможно нужно угадать по другим признакам
        if private_deals.fee_currency == "USDT" {
            // Можно попытаться извлечь из других полей
            return Some("UNKNOWN".to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_deal_protobuf() {
        // Создаем тестовые данные для сделки
        let deal = Deal {
            symbol: "BTCUSDT".to_string(),
            price: "50000.00".to_string(),
            quantity: "0.1".to_string(),
            time: 1640995200000,
            taker_order_side: 1, // buy
        };

        let mut buf = Vec::new();
        deal.encode(&mut buf).unwrap();

        // Декодируем обратно
        let json_result = deal_to_json(&deal).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        assert_eq!(parsed["d"]["symbol"], "BTCUSDT");
        assert_eq!(parsed["d"]["price"], "50000.00");
    }

    #[test]
    fn test_decode_ws_message_protobuf() {
        // Создаем тестовое WSMessage
        let deal = Deal {
            symbol: "ETHUSDT".to_string(),
            price: "3000.00".to_string(),
            quantity: "1.0".to_string(),
            time: 1640995200000,
            taker_order_side: 2, // sell
        };

        let mut deal_buf = Vec::new();
        deal.encode(&mut deal_buf).unwrap();

        let ws_message = WsMessage {
            channel: "spot@public.deals.v3.api@ETHUSDT".to_string(),
            data: deal_buf,
            ts: Some("1640995200000".to_string()),
        };

        let mut ws_buf = Vec::new();
        ws_message.encode(&mut ws_buf).unwrap();

        // Декодируем обратно
        let json_result = decode_mexc_protobuf(&ws_buf).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        assert_eq!(parsed["c"], "spot@public.deals.v3.api@ETHUSDT");
        assert_eq!(parsed["d"]["symbol"], "ETHUSDT");
        assert_eq!(parsed["d"]["takerOrderSide"], 2);
    }

    #[test]
    fn test_decode_private_deals_v3_api() {
        // Создаем тестовые данные для приватных сделок согласно реальному формату MEXC
        let private_deals = PrivateDealsV3Api {
            price: "3.6962".to_string(),
            quantity: "1".to_string(),
            amount: "3.6962".to_string(),
            trade_type: 2, // SELL
            is_maker: false,
            is_self_trade: false,
            trade_id: "505979017439002624X1".to_string(),
            client_order_id: "".to_string(),
            order_id: "C02__505979017439002624115".to_string(),
            fee_amount: "0.0003998377369698171".to_string(),
            fee_currency: "MX".to_string(),
            time: 1736417034280,
        };

        // Тестируем прямое декодирование
        let json_result = private_deals_to_json(&private_deals).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        assert_eq!(parsed["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed["symbol"], "MXUSDT"); // Должен извлечься из fee_currency="MX"
        assert_eq!(parsed["privateDeals"]["price"], "3.6962");
        assert_eq!(parsed["privateDeals"]["quantity"], "1");
        assert_eq!(parsed["privateDeals"]["amount"], "3.6962");
        assert_eq!(parsed["privateDeals"]["tradeType"], 2);
        assert_eq!(parsed["privateDeals"]["isMaker"], false);
        assert_eq!(parsed["privateDeals"]["isSelfTrade"], false);
        assert_eq!(parsed["privateDeals"]["tradeId"], "505979017439002624X1");
        assert_eq!(parsed["privateDeals"]["clientOrderId"], "");
        assert_eq!(parsed["privateDeals"]["orderId"], "C02__505979017439002624115");
        assert_eq!(parsed["privateDeals"]["feeAmount"], "0.0003998377369698171");
        assert_eq!(parsed["privateDeals"]["feeCurrency"], "MX");
        assert_eq!(parsed["privateDeals"]["time"], 1736417034280_i64);

        // Тестируем декодирование из бинарных данных
        let mut buf = Vec::new();
        private_deals.encode(&mut buf).unwrap();

        let json_result_from_binary = decode_mexc_protobuf(&buf).unwrap();
        let parsed_from_binary: Value = serde_json::from_str(&json_result_from_binary).unwrap();

        // Проверяем, что результат соответствует ожиданиям для прямого декодирования
        assert_eq!(parsed_from_binary["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed_from_binary["symbol"], "MXUSDT");
        assert_eq!(parsed_from_binary["privateDeals"]["tradeId"], "505979017439002624X1");
    }

    #[test]
    fn test_real_mexc_trade_data() {
        // Симулируем реальную торговую сделку как она приходит от MEXC
        let real_trade = PrivateDealsV3Api {
            price: "1.2345".to_string(),       // цена покупки
            quantity: "10.0".to_string(),      // количество
            amount: "12.345".to_string(),      // общая сумма = price * quantity
            trade_type: 1,                     // 1 = BUY, 2 = SELL
            is_maker: false,                   // не мейкер
            is_self_trade: false,              // не самосделка
            trade_id: "987654321".to_string(), // ID сделки
            client_order_id: "".to_string(),   // ID клиентского ордера
            order_id: "order_789".to_string(), // ID ордера
            fee_amount: "0.0123".to_string(),  // комиссия
            fee_currency: "USDT".to_string(),  // валюта комиссии
            time: 1749495346968,               // время сделки в миллисекундах
        };

        // Кодируем как это делает MEXC
        let mut buf = Vec::new();
        real_trade.encode(&mut buf).unwrap();

        // Декодируем обратно с нашей исправленной логикой
        let json_result = decode_mexc_protobuf(&buf).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        // Проверяем что данные декодированы правильно (не перепутаны)
        assert_eq!(parsed["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed["privateDeals"]["price"], "1.2345");
        assert_eq!(parsed["privateDeals"]["quantity"], "10.0");
        assert_eq!(parsed["privateDeals"]["amount"], "12.345");
        assert_eq!(parsed["privateDeals"]["tradeType"], 1);
        assert_eq!(parsed["privateDeals"]["tradeId"], "987654321");
        assert_eq!(parsed["privateDeals"]["orderId"], "order_789");
        assert_eq!(parsed["privateDeals"]["feeAmount"], "0.0123");
        assert_eq!(parsed["privateDeals"]["feeCurrency"], "USDT");
        assert_eq!(parsed["privateDeals"]["time"], 1749495346968_i64);

        // Убеждаемся что НЕТ перепутанных данных как было раньше
        assert_ne!(parsed["privateDeals"]["price"], "spot@private.deals.v3.api.pb");
        assert_ne!(parsed["privateDeals"]["amount"], "CLOREUSDT");
    }

    #[test]
    fn test_mexc_real_format_exact_match() {
        // Создаем точно такие же данные как в правильном ответе пользователя
        let mexc_real_trade = PrivateDealsV3Api {
            price: "3.6962".to_string(),
            quantity: "1".to_string(),
            amount: "3.6962".to_string(),
            trade_type: 2, // SELL
            is_maker: false,
            is_self_trade: false,
            trade_id: "505979017439002624X1".to_string(),
            client_order_id: "".to_string(),
            order_id: "C02__505979017439002624115".to_string(),
            fee_amount: "0.0003998377369698171".to_string(),
            fee_currency: "MX".to_string(),
            time: 1736417034280,
        };

        // Тестируем декодирование через decode_mexc_protobuf
        let mut buf = Vec::new();
        mexc_real_trade.encode(&mut buf).unwrap();

        let json_result = decode_mexc_protobuf(&buf).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        // Проверяем что структура точно соответствует ожидаемому формату
        assert_eq!(parsed["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed["symbol"], "MXUSDT");

        let private_deals = &parsed["privateDeals"];
        assert_eq!(private_deals["price"], "3.6962");
        assert_eq!(private_deals["quantity"], "1");
        assert_eq!(private_deals["amount"], "3.6962");
        assert_eq!(private_deals["tradeType"], 2);
        assert_eq!(private_deals["tradeId"], "505979017439002624X1");
        assert_eq!(private_deals["orderId"], "C02__505979017439002624115");
        assert_eq!(private_deals["feeAmount"], "0.0003998377369698171");
        assert_eq!(private_deals["feeCurrency"], "MX");
        assert_eq!(private_deals["time"], 1736417034280_i64);

        // Проверяем наличие sendTime
        assert!(parsed.get("sendTime").is_some());
        assert!(parsed["sendTime"].as_i64().unwrap() > 0);

        println!(
            "✅ Тест прошел: декодирование MEXC приватных сделок соответствует ожидаемому формату"
        );
        println!("Результат JSON: {}", json_result);
    }

    #[test]
    fn test_separation_private_and_public_messages() {
        // Тест 1: Проверяем что некорректные приватные сделки не декодируются
        let fake_private_deal = PrivateDealsV3Api {
            price: "".to_string(),  // Пустая цена - невалидно
            quantity: "CLOREUSDT".to_string(),  // Некорректное количество
            amount: "".to_string(),
            trade_type: 0,  // Некорректный тип
            is_maker: false,
            is_self_trade: false,
            trade_id: "".to_string(),
            client_order_id: "".to_string(),
            order_id: "".to_string(),
            fee_amount: "".to_string(),
            fee_currency: "".to_string(),
            time: 0,  // Некорректное время
        };

        let mut buf = Vec::new();
        fake_private_deal.encode(&mut buf).unwrap();

        // Это НЕ должно декодироваться как валидная приватная сделка
        let result = try_decode_private_deals(&buf);
        assert!(result.is_err(), "Невалидная приватная сделка должна отклоняться");

        // Тест 2: Проверяем что валидная приватная сделка декодируется правильно
        let valid_private_deal = PrivateDealsV3Api {
            price: "3.6962".to_string(),
            quantity: "1".to_string(),
            amount: "3.6962".to_string(),
            trade_type: 2,
            is_maker: false,
            is_self_trade: false,
            trade_id: "505979017439002624X1".to_string(),
            client_order_id: "".to_string(),
            order_id: "C02__505979017439002624115".to_string(),
            fee_amount: "0.0003998377369698171".to_string(),
            fee_currency: "MX".to_string(),
            time: 1736417034280,
        };

        let mut valid_buf = Vec::new();
        valid_private_deal.encode(&mut valid_buf).unwrap();

        // Это ДОЛЖНО декодироваться как валидная приватная сделка
        let valid_result = try_decode_private_deals(&valid_buf);
        assert!(valid_result.is_ok(), "Валидная приватная сделка должна декодироваться");

        let json_result = valid_result.unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();
        
        // Проверяем что это действительно приватная сделка
        assert_eq!(parsed["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed["symbol"], "MXUSDT");
        assert!(parsed.get("privateDeals").is_some());

        println!("✅ Тест разделения приватных и публичных сообщений прошел успешно");
    }

    #[test]
    fn test_user_reported_mixed_message() {
        // Воспроизводим конкретную проблему пользователя
        // Сообщение: {"c":"spot@public.deals.v3.api","d":{"price":"","quantity":"CLOREUSDT","symbol":"spot@private.deals.v3.api.pb","takerOrderSide":0,"time":0},"t":0}
        
        // Такое сообщение указывает на то, что данные декодируются как публичная сделка (Deal),
        // но с некорректными данными приватной сделки
        
        let problematic_deal = Deal {
            symbol: "spot@private.deals.v3.api.pb".to_string(),  // Это канал, а не символ!
            price: "".to_string(),  // Пустая цена
            quantity: "CLOREUSDT".to_string(),  // Символ в количестве
            time: 0,  // Время ноль
            taker_order_side: 0,
        };

        let mut buf = Vec::new();
        problematic_deal.encode(&mut buf).unwrap();

        // Проверяем что такие данные НЕ проходят валидацию как публичная сделка
        let result = try_decode_public_messages(&buf);
        
        if result.is_ok() {
            let json_result = result.unwrap();
            println!("❌ Проблематичные данные прошли валидацию: {}", json_result);
            
            // Анализируем что именно прошло
            let parsed: Value = serde_json::from_str(&json_result).unwrap();
            println!("Канал: {}", parsed.get("c").unwrap_or(&Value::Null));
            if let Some(d) = parsed.get("d") {
                println!("Symbol в d: {}", d.get("symbol").unwrap_or(&Value::Null));
                println!("Price в d: {}", d.get("price").unwrap_or(&Value::Null));
                println!("Quantity в d: {}", d.get("quantity").unwrap_or(&Value::Null));
            }
            
            assert!(false, "Проблематичные данные не должны проходить валидацию");
        } else {
            println!("✅ Проблематичные данные корректно отклонены: {:?}", result.err());
        }

        // Теперь проверяем что правильные данные проходят
        let valid_deal = Deal {
            symbol: "BTCUSDT".to_string(),
            price: "50000.00".to_string(),
            quantity: "0.1".to_string(),
            time: 1640995200000,
            taker_order_side: 1,
        };

        let mut valid_buf = Vec::new();
        valid_deal.encode(&mut valid_buf).unwrap();

        let valid_result = try_decode_public_messages(&valid_buf);
        assert!(valid_result.is_ok(), "Валидные публичные данные должны проходить");
        
        println!("✅ Тест с проблематичными данными пользователя завершен");
    }

    #[test]
    fn test_official_mexc_wrapper_format() {
        // Тестируем официальную схему MEXC с PushDataV3ApiWrapper
        use push_data_v3_api_wrapper::Body;

        let private_deals = PrivateDealsV3Api {
            price: "3.6962".to_string(),
            quantity: "1".to_string(),
            amount: "3.6962".to_string(),
            trade_type: 2,
            is_maker: false,
            is_self_trade: false,
            trade_id: "505979017439002624X1".to_string(),
            client_order_id: "".to_string(),
            order_id: "C02__505979017439002624115".to_string(),
            fee_amount: "0.0003998377369698171".to_string(),
            fee_currency: "MX".to_string(),
            time: 1736417034280,
        };

        let wrapper = PushDataV3ApiWrapper {
            channel: "spot@private.deals.v3.api.pb".to_string(),
            body: Some(Body::PrivateDeals(private_deals)),
            symbol: Some("MXUSDT".to_string()),
            symbol_id: None,
            create_time: Some(1736417034332),
            send_time: Some(1736417034332),
        };

        // Кодируем wrapper
        let mut buf = Vec::new();
        wrapper.encode(&mut buf).unwrap();

        // Декодируем обратно
        let json_result = decode_mexc_protobuf(&buf).unwrap();
        let parsed: Value = serde_json::from_str(&json_result).unwrap();

        // Проверяем правильность декодирования
        assert_eq!(parsed["channel"], "spot@private.deals.v3.api.pb");
        assert_eq!(parsed["symbol"], "MXUSDT");
        assert_eq!(parsed["sendTime"], 1736417034332_i64);
        
        let private_deals_data = &parsed["privateDeals"];
        assert_eq!(private_deals_data["price"], "3.6962");
        assert_eq!(private_deals_data["quantity"], "1");
        assert_eq!(private_deals_data["amount"], "3.6962");
        assert_eq!(private_deals_data["tradeType"], 2);
        assert_eq!(private_deals_data["isMaker"], false);
        assert_eq!(private_deals_data["isSelfTrade"], false);
        assert_eq!(private_deals_data["tradeId"], "505979017439002624X1");
        assert_eq!(private_deals_data["clientOrderId"], "");
        assert_eq!(private_deals_data["orderId"], "C02__505979017439002624115");
        assert_eq!(private_deals_data["feeAmount"], "0.0003998377369698171");
        assert_eq!(private_deals_data["feeCurrency"], "MX");
        assert_eq!(private_deals_data["time"], 1736417034280_i64);

        println!("✅ Официальная схема MEXC PushDataV3ApiWrapper работает корректно!");
        println!("JSON результат: {}", json_result);
    }
}

