//! WebSocket message models for the Trading SDK.
//!
//! NOTE: This module is temporarily added here until we have a better solution.
//! This enum is only used in the trading-sdk for deserializing server messages.
//! It does NOT live in trading-api-types because:
//! 1. The server uses optimized types with `&'static str` and `Arc<str>` that can't deserialize
//! 2. This struct is only needed by SDK clients, not the server
//!
//! IMPORTANT: When new message types are added to the server, they must be manually
//! added to the `ServerMessage` enum below.

use crate::types::{
    AggTradeMessage, BookTickerMessage, DepthUpdate, ErrorMessage, ForceOrderMessage,
    MarkPriceMessage, OrderUpdateMessage, PongMessage, RequestId, StatusMessage,
};
use serde::Deserialize;

/// Result message for subscribe/unsubscribe success
#[derive(Deserialize, Clone, Debug)]
pub struct MethodResult {
    #[serde(default)]
    pub id: Option<RequestId>,
    /// Event time (ms)
    #[serde(rename = "E")]
    pub event_time: u64,
    pub result: String,
}

/// Result message for list_subscriptions
#[derive(Deserialize, Clone, Debug)]
pub struct ListSubscriptionsResult {
    #[serde(default)]
    pub id: Option<RequestId>,
    /// Event time (ms)
    #[serde(rename = "E")]
    pub event_time: u64,
    pub result: Vec<String>,
}

/// Tagged messages from the server (have an "e" event type field)
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "e", rename_all = "snake_case")]
pub enum TaggedMessage {
    Status(StatusMessage),
    Pong(PongMessage),
    Error(ErrorMessage),
    Subscribe(MethodResult),
    Unsubscribe(MethodResult),
    ListSubscriptions(ListSubscriptionsResult),
}

/// All possible server messages.
///
/// Uses untagged deserialization - serde tries each variant in order until one matches.
/// The `Unknown` variant captures any message that doesn't match known types.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ServerMessage {
    // Tagged messages with "e" field - try these first
    Tagged(TaggedMessage),

    // Binance-style messages (identified by "e" event type field)
    DepthUpdate(DepthUpdate),
    AggTrade(AggTradeMessage),
    BookTicker(BookTickerMessage),
    MarkPrice(MarkPriceMessage),
    ForceOrder(ForceOrderMessage),
    OrderUpdate(OrderUpdateMessage),

    // Untagged error response (e.g., order errors without "e" field)
    Error(ErrorMessage),

    /// Failed to parse message - contains (error message, raw text)
    #[serde(skip)]
    Unknown(String, String),
}

impl ServerMessage {
    /// Returns true if this is an error message
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ServerMessage::Tagged(TaggedMessage::Error(_)) | ServerMessage::Error(_)
        )
    }

    /// Returns the request ID if present
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            ServerMessage::Tagged(msg) => match msg {
                TaggedMessage::Pong(m) => m.id,
                TaggedMessage::Error(m) => m.id,
                TaggedMessage::Subscribe(m) => m.id,
                TaggedMessage::Unsubscribe(m) => m.id,
                TaggedMessage::ListSubscriptions(m) => m.id,
                _ => None,
            },
            ServerMessage::Error(m) => m.id,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_update() {
        let json = r#"{
            "e": "depthUpdate",
            "E": 1234567890,
            "T": 1234567890,
            "s": "BTCUSDT",
            "U": 100,
            "u": 200,
            "pu": 99,
            "b": [["50000.00", "1.5"]],
            "a": [["50001.00", "2.0"]],
            "mt": "s"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::DepthUpdate(_)));

        if let ServerMessage::DepthUpdate(d) = msg {
            assert_eq!(d.symbol, "BTCUSDT");
            assert_eq!(d.bids.len(), 1);
            assert_eq!(d.asks.len(), 1);
        }
    }

    #[test]
    fn test_agg_trade() {
        let json = r#"{
            "e": "aggTrade",
            "E": 1234567890,
            "s": "BTCUSDT",
            "a": 12345,
            "p": "50000.00",
            "q": "1.5",
            "f": 100,
            "l": 105,
            "T": 1234567890,
            "m": true,
            "th": "0xabc123",
            "ua": "0xdef456",
            "oi": 999,
            "mk": true,
            "ff": false,
            "lq": false,
            "fe": "0.001",
            "nf": "0.001",
            "fa": "USDT",
            "sd": "BUY"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::AggTrade(_)));

        if let ServerMessage::AggTrade(t) = msg {
            assert_eq!(t.symbol, "BTCUSDT");
            assert_eq!(t.price, "50000.00");
            assert!(t.is_buyer_maker);
        }
    }

    #[test]
    fn test_book_ticker() {
        let json = r#"{
            "e": "bookTicker",
            "u": 12345,
            "E": 1234567890,
            "T": 1234567890,
            "s": "ETHUSDT",
            "b": "3000.00",
            "B": "10.5",
            "a": "3001.00",
            "A": "8.2",
            "mt": "u"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::BookTicker(_)));

        if let ServerMessage::BookTicker(b) = msg {
            assert_eq!(b.symbol, "ETHUSDT");
            assert_eq!(b.best_bid_price, "3000.00");
            assert_eq!(b.best_ask_price, "3001.00");
        }
    }

    #[test]
    fn test_mark_price() {
        let json = r#"{
            "e": "markPriceUpdate",
            "E": 1234567890,
            "s": "BTCUSDT",
            "p": "50000.00",
            "i": "49999.00",
            "r": "0.0001"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::MarkPrice(_)));

        if let ServerMessage::MarkPrice(m) = msg {
            assert_eq!(m.symbol, "BTCUSDT");
            assert_eq!(m.mark_price, "50000.00");
            assert_eq!(m.funding_rate, "0.0001");
        }
    }

    #[test]
    fn test_force_order() {
        let json = r#"{
            "e": "liquidation",
            "E": 1234567890,
            "o": {
                "s": "BTCUSDT",
                "S": "SELL",
                "o": "LIMIT",
                "f": "IOC",
                "p": "49000.00",
                "ap": "49000.00",
                "X": "FILLED",
                "l": "1.0",
                "T": 1234567890,
                "th": "0xabc",
                "ua": "0xdef",
                "oi": 123,
                "ti": 456
            }
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::ForceOrder(_)));

        if let ServerMessage::ForceOrder(f) = msg {
            assert_eq!(f.order.symbol, "BTCUSDT");
            assert_eq!(f.order.side, "SELL");
        }
    }

    #[test]
    fn test_order_update() {
        let json = r#"{
            "e": "orderTradeUpdate",
            "E": 1234567890,
            "o": {
                "s": "BTCUSDT",
                "i": 12345,
                "X": "NEW",
                "x": "NEW",
                "T": 1234567890,
                "th": "0xabc",
                "ua": "0xdef",
                "S": "BUY",
                "o": "LIMIT",
                "f": "GTC",
                "p": "50000.00",
                "q": "1.0"
            }
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::OrderUpdate(_)));

        if let ServerMessage::OrderUpdate(o) = msg {
            assert_eq!(o.event_time, 1234567890);
        }
    }

    #[test]
    fn test_status_message() {
        let json = r#"{
            "e": "status",
            "E": 1234567890,
            "status": "connected",
            "clientId": "client-123"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ServerMessage::Tagged(TaggedMessage::Status(_))
        ));

        if let ServerMessage::Tagged(TaggedMessage::Status(s)) = msg {
            assert_eq!(s.status, "connected");
            assert_eq!(s.client_id, "client-123");
            assert_eq!(s.event_time, 1234567890);
        }
    }

    #[test]
    fn test_pong_message() {
        let json = r#"{
            "e": "pong",
            "id": 42,
            "E": 1234567890
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::Tagged(TaggedMessage::Pong(_))));
        assert_eq!(msg.request_id(), Some(RequestId::from(42)));
    }

    #[test]
    fn test_error_message() {
        let json = r#"{
            "e": "error",
            "id": 1,
            "E": 1234567890,
            "error": {
                "code": -1004,
                "msg": "Invalid subscription format"
            }
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(msg.is_error(), "Expected error message, got: {msg:?}");
        assert_eq!(msg.request_id(), Some(RequestId::from(1)));
    }

    #[test]
    fn test_order_error() {
        // Order errors come without the "e" tag
        let json = r#"{
            "id": 2,
            "E": 1234567890,
            "error": {
                "code": -2010,
                "msg": "Transaction execution unsuccessful"
            }
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(msg.is_error(), "Expected error message, got: {msg:?}");
        assert_eq!(msg.request_id(), Some(RequestId::from(2)));
        assert!(matches!(msg, ServerMessage::Error(_)));
    }

    #[test]
    fn test_subscribe_success() {
        let json = r#"{
            "e": "subscribe",
            "id": 5,
            "E": 1234567890,
            "result": "success"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ServerMessage::Tagged(TaggedMessage::Subscribe(_))
        ));
        assert_eq!(msg.request_id(), Some(RequestId::from(5)));

        if let ServerMessage::Tagged(TaggedMessage::Subscribe(s)) = msg {
            assert_eq!(s.result, "success");
        }
    }

    #[test]
    fn test_unsubscribe_success() {
        let json = r#"{
            "e": "unsubscribe",
            "id": 6,
            "E": 1234567890,
            "result": "success"
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ServerMessage::Tagged(TaggedMessage::Unsubscribe(_))
        ));
        assert_eq!(msg.request_id(), Some(RequestId::from(6)));
    }

    #[test]
    fn test_list_subscriptions() {
        let json = r#"{
            "e": "list_subscriptions",
            "id": 7,
            "E": 1234567890,
            "result": ["btcusdt@depth10", "ethusdt@aggTrade"]
        }"#;

        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(
            msg,
            ServerMessage::Tagged(TaggedMessage::ListSubscriptions(_))
        ));

        if let ServerMessage::Tagged(TaggedMessage::ListSubscriptions(l)) = msg {
            assert_eq!(l.result.len(), 2);
            assert_eq!(l.result[0], "btcusdt@depth10");
        }
    }
}
