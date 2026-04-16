//! Typed parsing helpers for string-encoded prices and quantities.
//!
//! REST and WebSocket responses return prices and quantities as strings
//! (Binance FAPI convention). This module provides extension traits for
//! parsing them into `rust_decimal::Decimal` without loss of precision.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::ParseDecimal;
//!
//! // On WebSocket depth updates
//! for level in &depth.bids {
//!     let (price, qty) = level.parse_decimal()?;
//!     println!("bid {price} x {qty}");
//! }
//!
//! // On WebSocket book ticker
//! let (bid_px, bid_qty) = bt.best_bid()?;
//! ```

use rust_decimal::Decimal;

use bullet_ws_interface::{
    AggTradeMessage, BookTickerMessage, DepthUpdate, MarkPriceMessage, PriceLevel,
};

/// Extension trait for parsing string-encoded prices and quantities into `Decimal`.
pub trait ParseDecimal {
    /// Parse the price and quantity as `(Decimal, Decimal)`.
    fn parse_decimal(&self) -> Result<(Decimal, Decimal), rust_decimal::Error>;
}

impl ParseDecimal for PriceLevel {
    fn parse_decimal(&self) -> Result<(Decimal, Decimal), rust_decimal::Error> {
        Ok((self.0.parse()?, self.1.parse()?))
    }
}

/// Typed orderbook price level.
#[derive(Debug, Clone, Copy)]
pub struct TypedLevel {
    pub price: Decimal,
    pub qty: Decimal,
}

/// Typed orderbook with `Decimal` prices and quantities.
#[derive(Debug, Clone)]
pub struct TypedOrderBook {
    pub bids: Vec<TypedLevel>,
    pub asks: Vec<TypedLevel>,
    pub last_update_id: u64,
}

/// Extension methods for [`DepthUpdate`] (WebSocket).
pub trait DepthUpdateExt {
    /// Parse all bid levels into `Decimal` pairs.
    fn typed_bids(&self) -> Result<Vec<TypedLevel>, rust_decimal::Error>;
    /// Parse all ask levels into `Decimal` pairs.
    fn typed_asks(&self) -> Result<Vec<TypedLevel>, rust_decimal::Error>;
}

impl DepthUpdateExt for DepthUpdate {
    fn typed_bids(&self) -> Result<Vec<TypedLevel>, rust_decimal::Error> {
        self.bids
            .iter()
            .map(|l| {
                let (price, qty) = l.parse_decimal()?;
                Ok(TypedLevel { price, qty })
            })
            .collect()
    }

    fn typed_asks(&self) -> Result<Vec<TypedLevel>, rust_decimal::Error> {
        self.asks
            .iter()
            .map(|l| {
                let (price, qty) = l.parse_decimal()?;
                Ok(TypedLevel { price, qty })
            })
            .collect()
    }
}

/// Extension methods for [`BookTickerMessage`] (WebSocket BBO).
pub trait BookTickerExt {
    /// Parse best bid as `(price, quantity)`.
    fn best_bid(&self) -> Result<(Decimal, Decimal), rust_decimal::Error>;
    /// Parse best ask as `(price, quantity)`.
    fn best_ask(&self) -> Result<(Decimal, Decimal), rust_decimal::Error>;
}

impl BookTickerExt for BookTickerMessage {
    fn best_bid(&self) -> Result<(Decimal, Decimal), rust_decimal::Error> {
        Ok((self.best_bid_price.parse()?, self.best_bid_qty.parse()?))
    }

    fn best_ask(&self) -> Result<(Decimal, Decimal), rust_decimal::Error> {
        Ok((self.best_ask_price.parse()?, self.best_ask_qty.parse()?))
    }
}

/// Extension methods for [`AggTradeMessage`] (WebSocket trades).
pub trait AggTradeExt {
    /// Parse the trade price.
    fn parsed_price(&self) -> Result<Decimal, rust_decimal::Error>;
    /// Parse the trade quantity.
    fn parsed_quantity(&self) -> Result<Decimal, rust_decimal::Error>;
}

impl AggTradeExt for AggTradeMessage {
    fn parsed_price(&self) -> Result<Decimal, rust_decimal::Error> {
        self.price.parse()
    }

    fn parsed_quantity(&self) -> Result<Decimal, rust_decimal::Error> {
        self.quantity.parse()
    }
}

/// Extension methods for [`MarkPriceMessage`] (WebSocket mark price + funding).
pub trait MarkPriceExt {
    /// Parse the mark price.
    fn parsed_mark_price(&self) -> Result<Decimal, rust_decimal::Error>;
    /// Parse the index price.
    fn parsed_index_price(&self) -> Result<Decimal, rust_decimal::Error>;
    /// Parse the funding rate.
    fn parsed_funding_rate(&self) -> Result<Decimal, rust_decimal::Error>;
}

impl MarkPriceExt for MarkPriceMessage {
    fn parsed_mark_price(&self) -> Result<Decimal, rust_decimal::Error> {
        self.mark_price.parse()
    }

    fn parsed_index_price(&self) -> Result<Decimal, rust_decimal::Error> {
        self.index_price.parse()
    }

    fn parsed_funding_rate(&self) -> Result<Decimal, rust_decimal::Error> {
        self.funding_rate.parse()
    }
}

/// Parse a REST orderbook response (`Vec<Vec<String>>` levels) into typed levels.
pub fn parse_levels(
    raw: &[Vec<String>],
) -> Result<Vec<TypedLevel>, rust_decimal::Error> {
    raw.iter()
        .filter(|l| l.len() >= 2)
        .map(|l| {
            Ok(TypedLevel {
                price: l[0].parse()?,
                qty: l[1].parse()?,
            })
        })
        .collect()
}
