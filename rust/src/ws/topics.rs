//! Typed constructors for WebSocket subscription strings.

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

    /// User order stream for an address.
    UserOrders { address: String },

    /// Completed Degen position stream for an address.
    UserDegenPositions { address: String },

    /// All symbols mini ticker stream.
    AllTickers,

    /// All symbols mark price stream.
    AllMarkPrices,

    /// All symbols book ticker stream.
    AllBookTickers,

    /// All symbols liquidation stream.
    AllForceOrders,

    /// Trading status of every market: a snapshot on subscribe, deltas after.
    AllMarketStatuses,
}

impl Topic {
    /// Subscribe to aggregated trades for a symbol.
    pub fn agg_trade(symbol: impl Into<String>) -> Self {
        Self::AggTrade {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to order book depth for a symbol.
    pub fn depth(symbol: impl Into<String>, depth: OrderbookDepth) -> Self {
        Self::Depth {
            symbol: symbol.into(),
            depth,
        }
    }

    /// Subscribe to best bid/ask for a symbol.
    pub fn book_ticker(symbol: impl Into<String>) -> Self {
        Self::BookTicker {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to mark price updates for a symbol.
    pub fn mark_price(symbol: impl Into<String>) -> Self {
        Self::MarkPrice {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to kline/candlestick data for a symbol.
    pub fn kline(symbol: impl Into<String>, interval: KlineInterval) -> Self {
        Self::Kline {
            symbol: symbol.into(),
            interval,
        }
    }

    /// Subscribe to liquidation orders for a symbol.
    pub fn force_order(symbol: impl Into<String>) -> Self {
        Self::ForceOrder {
            symbol: symbol.into(),
        }
    }

    /// Subscribe to user order updates for an address.
    pub fn user_orders(address: impl Into<String>) -> Self {
        Self::UserOrders {
            address: address.into(),
        }
    }

    /// Subscribe to completed Degen positions for an address.
    pub fn user_degen_positions(address: impl Into<String>) -> Self {
        Self::UserDegenPositions {
            address: address.into(),
        }
    }

    /// Subscribe to mini ticker updates for all symbols.
    pub fn all_tickers() -> Self {
        Self::AllTickers
    }

    /// Subscribe to mark price updates for all symbols.
    pub fn all_mark_prices() -> Self {
        Self::AllMarkPrices
    }

    /// Subscribe to book ticker updates for all symbols.
    pub fn all_book_tickers() -> Self {
        Self::AllBookTickers
    }

    /// Subscribe to liquidation orders for all symbols.
    pub fn all_force_orders() -> Self {
        Self::AllForceOrders
    }

    /// Subscribe to the trading status of all markets.
    pub fn all_market_statuses() -> Self {
        Self::AllMarketStatuses
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
            Topic::UserOrders { address } => write!(f, "{address}@user.orders"),
            Topic::UserDegenPositions { address } => {
                write!(f, "{address}@user.degenPositions")
            }
            Topic::AllTickers => write!(f, "!ticker@arr"),
            Topic::AllMarkPrices => write!(f, "!markPrice@arr"),
            Topic::AllBookTickers => write!(f, "!bookTicker@arr"),
            Topic::AllForceOrders => write!(f, "!forceOrder@arr"),
            Topic::AllMarketStatuses => write!(f, "!marketStatus@arr"),
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
    fn topic_wire_format() {
        for (topic, want) in [
            (Topic::agg_trade("BTC-USD"), "BTC-USD@aggTrade"),
            (
                Topic::depth("BTC-USD", OrderbookDepth::D5),
                "BTC-USD@depth5",
            ),
            (
                Topic::depth("BTC-USD", OrderbookDepth::D10),
                "BTC-USD@depth10",
            ),
            (
                Topic::depth("BTC-USD", OrderbookDepth::D20),
                "BTC-USD@depth20",
            ),
            (Topic::book_ticker("ETH-USD"), "ETH-USD@bookTicker"),
            (Topic::mark_price("SOL-USD"), "SOL-USD@markPrice"),
            (
                Topic::kline("BTC-USD", KlineInterval::M1),
                "BTC-USD@kline_1m",
            ),
            (
                Topic::kline("BTC-USD", KlineInterval::H4),
                "BTC-USD@kline_4h",
            ),
            (
                Topic::kline("BTC-USD", KlineInterval::D1),
                "BTC-USD@kline_1d",
            ),
            (Topic::force_order("BTC-USD"), "BTC-USD@forceOrder"),
            (Topic::user_orders("0xabc"), "0xabc@user.orders"),
            (
                Topic::user_degen_positions("0xabc"),
                "0xabc@user.degenPositions",
            ),
            (Topic::all_tickers(), "!ticker@arr"),
            (Topic::all_mark_prices(), "!markPrice@arr"),
            (Topic::all_market_statuses(), "!marketStatus@arr"),
            (Topic::all_book_tickers(), "!bookTicker@arr"),
            (Topic::all_force_orders(), "!forceOrder@arr"),
        ] {
            assert_eq!(topic.to_string(), want);
            assert_eq!(String::from(topic), want);
        }
    }
}
