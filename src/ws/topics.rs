//! Strongly-typed WebSocket subscription topics.
//!
//! This module provides type-safe topic builders for WebSocket subscriptions,
//! eliminating the need to remember string formats like `"BTC-USD@depth10"`.
//!
//! # Example
//!
//! ```no_run
//! use bullet_rust_sdk::ws::topics::{Topic, OrderbookDepth, KlineInterval};
//! use trading_api_types::RequestId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let api = bullet_rust_sdk::TradingApi::mainnet().await?;
//! # let mut ws = api.connect_ws().await?;
//! // Type-safe subscriptions
//! ws.subscribe([
//!     Topic::agg_trade("BTC-USD"),
//!     Topic::depth("ETH-USD", OrderbookDepth::D10),
//!     Topic::book_ticker("SOL-USD"),
//!     Topic::kline("BTC-USD", KlineInterval::H1),
//! ], Some(RequestId::new(1))).await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;

/// Orderbook depth levels for depth subscriptions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrderbookDepth {
    /// 5 levels
    D5,
    /// 10 levels (default)
    #[default]
    D10,
    /// 20 levels
    D20,
}

impl OrderbookDepth {
    fn as_str(&self) -> &'static str {
        match self {
            OrderbookDepth::D5 => "5",
            OrderbookDepth::D10 => "10",
            OrderbookDepth::D20 => "20",
        }
    }
}

/// Kline (candlestick) intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KlineInterval {
    /// 1 minute
    M1,
    /// 5 minutes
    M5,
    /// 15 minutes
    M15,
    /// 30 minutes
    M30,
    /// 1 hour
    H1,
    /// 4 hours
    H4,
    /// 1 day
    D1,
}

impl KlineInterval {
    fn as_str(&self) -> &'static str {
        match self {
            KlineInterval::M1 => "1m",
            KlineInterval::M5 => "5m",
            KlineInterval::M15 => "15m",
            KlineInterval::M30 => "30m",
            KlineInterval::H1 => "1h",
            KlineInterval::H4 => "4h",
            KlineInterval::D1 => "1d",
        }
    }
}

/// A WebSocket subscription topic.
///
/// Topics are created using the static constructor methods and converted to
/// the wire format automatically when passed to [`subscribe()`](super::WebsocketHandle::subscribe).
///
/// # Available Topics
///
/// | Topic | Description |
/// |-------|-------------|
/// | [`Topic::agg_trade`] | Aggregated trade updates |
/// | [`Topic::depth`] | Order book depth snapshots |
/// | [`Topic::book_ticker`] | Best bid/ask prices |
/// | [`Topic::mark_price`] | Mark price updates |
/// | [`Topic::kline`] | Candlestick/kline data |
/// | [`Topic::force_order`] | Liquidation orders |
/// | [`Topic::all_tickers`] | All symbol mini tickers |
/// | [`Topic::all_mark_prices`] | All mark prices |
/// | [`Topic::all_book_tickers`] | All best bid/ask |
/// | [`Topic::all_force_orders`] | All liquidations |
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    /// Aggregated trade stream for a symbol.
    AggTrade { symbol: String },

    /// Order book depth stream with configurable levels.
    Depth {
        symbol: String,
        depth: OrderbookDepth,
    },

    /// Best bid/ask stream for a symbol.
    BookTicker { symbol: String },

    /// Mark price stream for a symbol.
    MarkPrice { symbol: String },

    /// Kline/candlestick stream for a symbol at an interval.
    Kline {
        symbol: String,
        interval: KlineInterval,
    },

    /// Liquidation order stream for a symbol.
    ForceOrder { symbol: String },

    /// All symbols mini ticker stream.
    AllTickers,

    /// All symbols mark price stream.
    AllMarkPrices,

    /// All symbols book ticker stream.
    AllBookTickers,

    /// All symbols liquidation stream.
    AllForceOrders,
}

impl Topic {
    /// Subscribe to aggregated trades for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::agg_trade("BTC-USD");
    /// assert_eq!(topic.to_string(), "BTC-USD@aggTrade");
    /// ```
    pub fn agg_trade(symbol: impl Into<String>) -> Self {
        Self::AggTrade {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to order book depth for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::{Topic, OrderbookDepth};
    ///
    /// let topic = Topic::depth("BTC-USD", OrderbookDepth::D10);
    /// assert_eq!(topic.to_string(), "BTC-USD@depth10");
    /// ```
    pub fn depth(symbol: impl Into<String>, depth: OrderbookDepth) -> Self {
        Self::Depth {
            symbol: symbol.into(),
            depth,
        }
    }

    /// Subscribe to best bid/ask for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::book_ticker("BTC-USD");
    /// assert_eq!(topic.to_string(), "BTC-USD@bookTicker");
    /// ```
    pub fn book_ticker(symbol: impl Into<String>) -> Self {
        Self::BookTicker {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to mark price updates for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::mark_price("BTC-USD");
    /// assert_eq!(topic.to_string(), "BTC-USD@markPrice");
    /// ```
    pub fn mark_price(symbol: impl Into<String>) -> Self {
        Self::MarkPrice {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to kline/candlestick data for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::{Topic, KlineInterval};
    ///
    /// let topic = Topic::kline("BTC-USD", KlineInterval::H1);
    /// assert_eq!(topic.to_string(), "BTC-USD@kline_1h");
    /// ```
    pub fn kline(symbol: impl Into<String>, interval: KlineInterval) -> Self {
        Self::Kline {
            symbol: symbol.into(),
            interval,
        }
    }

    /// Subscribe to liquidation orders for a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::force_order("BTC-USD");
    /// assert_eq!(topic.to_string(), "BTC-USD@forceOrder");
    /// ```
    pub fn force_order(symbol: impl Into<String>) -> Self {
        Self::ForceOrder {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to mini ticker updates for all symbols.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::all_tickers();
    /// assert_eq!(topic.to_string(), "!ticker@arr");
    /// ```
    pub fn all_tickers() -> Self {
        Self::AllTickers
    }

    /// Subscribe to mark price updates for all symbols.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::all_mark_prices();
    /// assert_eq!(topic.to_string(), "!markPrice@arr");
    /// ```
    pub fn all_mark_prices() -> Self {
        Self::AllMarkPrices
    }

    /// Subscribe to book ticker updates for all symbols.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::all_book_tickers();
    /// assert_eq!(topic.to_string(), "!bookTicker@arr");
    /// ```
    pub fn all_book_tickers() -> Self {
        Self::AllBookTickers
    }

    /// Subscribe to liquidation orders for all symbols.
    ///
    /// # Example
    ///
    /// ```
    /// use bullet_rust_sdk::ws::topics::Topic;
    ///
    /// let topic = Topic::all_force_orders();
    /// assert_eq!(topic.to_string(), "!forceOrder@arr");
    /// ```
    pub fn all_force_orders() -> Self {
        Self::AllForceOrders
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Topic::AggTrade { symbol } => write!(f, "{symbol}@aggTrade"),
            Topic::Depth { symbol, depth } => write!(f, "{symbol}@depth{}", depth.as_str()),
            Topic::BookTicker { symbol } => write!(f, "{symbol}@bookTicker"),
            Topic::MarkPrice { symbol } => write!(f, "{symbol}@markPrice"),
            Topic::Kline { symbol, interval } => write!(f, "{symbol}@kline_{}", interval.as_str()),
            Topic::ForceOrder { symbol } => write!(f, "{symbol}@forceOrder"),
            Topic::AllTickers => write!(f, "!ticker@arr"),
            Topic::AllMarkPrices => write!(f, "!markPrice@arr"),
            Topic::AllBookTickers => write!(f, "!bookTicker@arr"),
            Topic::AllForceOrders => write!(f, "!forceOrder@arr"),
        }
    }
}

impl From<Topic> for String {
    fn from(topic: Topic) -> Self {
        topic.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agg_trade() {
        assert_eq!(Topic::agg_trade("BTC-USD").to_string(), "BTC-USD@aggTrade");
    }

    #[test]
    fn test_depth() {
        assert_eq!(
            Topic::depth("BTC-USD", OrderbookDepth::D5).to_string(),
            "BTC-USD@depth5"
        );
        assert_eq!(
            Topic::depth("BTC-USD", OrderbookDepth::D10).to_string(),
            "BTC-USD@depth10"
        );
        assert_eq!(
            Topic::depth("BTC-USD", OrderbookDepth::D20).to_string(),
            "BTC-USD@depth20"
        );
    }

    #[test]
    fn test_book_ticker() {
        assert_eq!(
            Topic::book_ticker("ETH-USD").to_string(),
            "ETH-USD@bookTicker"
        );
    }

    #[test]
    fn test_mark_price() {
        assert_eq!(
            Topic::mark_price("SOL-USD").to_string(),
            "SOL-USD@markPrice"
        );
    }

    #[test]
    fn test_kline() {
        assert_eq!(
            Topic::kline("BTC-USD", KlineInterval::M1).to_string(),
            "BTC-USD@kline_1m"
        );
        assert_eq!(
            Topic::kline("BTC-USD", KlineInterval::H4).to_string(),
            "BTC-USD@kline_4h"
        );
        assert_eq!(
            Topic::kline("BTC-USD", KlineInterval::D1).to_string(),
            "BTC-USD@kline_1d"
        );
    }

    #[test]
    fn test_force_order() {
        assert_eq!(
            Topic::force_order("BTC-USD").to_string(),
            "BTC-USD@forceOrder"
        );
    }

    #[test]
    fn test_all_streams() {
        assert_eq!(Topic::all_tickers().to_string(), "!ticker@arr");
        assert_eq!(Topic::all_mark_prices().to_string(), "!markPrice@arr");
        assert_eq!(Topic::all_book_tickers().to_string(), "!bookTicker@arr");
        assert_eq!(Topic::all_force_orders().to_string(), "!forceOrder@arr");
    }

    #[test]
    fn test_into_string() {
        let topic = Topic::agg_trade("BTC-USD");
        let s: String = topic.into();
        assert_eq!(s, "BTC-USD@aggTrade");
    }
}
