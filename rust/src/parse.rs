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
///
/// Construct via [`parse_order_book`].
#[derive(Debug, Clone)]
pub struct TypedOrderBook {
    pub bids: Vec<TypedLevel>,
    pub asks: Vec<TypedLevel>,
    pub last_update_id: u64,
}

/// Parse a REST `OrderBook` response into typed `Decimal` levels.
///
/// ```ignore
/// let book = client.order_book(Some(20), "BTC-USD").await?.into_inner();
/// let typed = parse_order_book(&book.bids, &book.asks, book.last_update_id)?;
/// ```
pub fn parse_order_book(
    bids: &[Vec<String>],
    asks: &[Vec<String>],
    last_update_id: u64,
) -> Result<TypedOrderBook, rust_decimal::Error> {
    Ok(TypedOrderBook {
        bids: parse_levels(bids)?,
        asks: parse_levels(asks)?,
        last_update_id,
    })
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

/// Extension methods for REST [`BinanceOrder`](crate::codegen::types::BinanceOrder) responses.
///
/// The generated `BinanceOrder` has `side`, `order_type`, and `time_in_force` as strings.
/// The API follows Binance convention: the Bullet `OrderType` is encoded as a
/// `(order_type, time_in_force)` pair — e.g. `PostOnly` → `("LIMIT", "GTX")`.
pub trait BinanceOrderExt {
    /// Parse side string (`"BUY"` / `"SELL"`) into [`Side`](bullet_exchange_interface::types::Side).
    fn parsed_side(&self) -> Option<bullet_exchange_interface::types::Side>;

    /// Derive the Bullet [`OrderType`](bullet_exchange_interface::types::OrderType) from
    /// the `order_type` + `time_in_force` string pair.
    ///
    /// | order_type | time_in_force | Result |
    /// |------------|---------------|--------|
    /// | `"LIMIT"`  | `"GTC"`       | `Limit` |
    /// | `"LIMIT"`  | `"GTX"`       | `PostOnly` |
    /// | `"LIMIT"`  | `"IOC"`       | `ImmediateOrCancel` |
    /// | `"LIMIT"`  | `"FOK"`       | `FillOrKill` |
    fn parsed_order_type(&self) -> Option<bullet_exchange_interface::types::OrderType>;
}

impl BinanceOrderExt for crate::generated::types::BinanceOrder {
    fn parsed_side(&self) -> Option<bullet_exchange_interface::types::Side> {
        match self.side.as_str() {
            "BUY" => Some(bullet_exchange_interface::types::Side::Bid),
            "SELL" => Some(bullet_exchange_interface::types::Side::Ask),
            _ => None,
        }
    }

    fn parsed_order_type(&self) -> Option<bullet_exchange_interface::types::OrderType> {
        use bullet_exchange_interface::types::OrderType;
        match (self.order_type.as_str(), self.time_in_force.as_str()) {
            ("LIMIT", "GTC") => Some(OrderType::Limit),
            ("LIMIT", "GTX") => Some(OrderType::PostOnly),
            ("LIMIT", "IOC") => Some(OrderType::ImmediateOrCancel),
            ("LIMIT", "FOK") => Some(OrderType::FillOrKill),
            _ => None,
        }
    }
}

/// Parse a REST orderbook response (`Vec<Vec<String>>` levels) into typed levels.
///
/// Entries with fewer than 2 elements are silently skipped.
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn parse_price_level() {
        let level = PriceLevel("50000.50".into(), "1.234".into());
        let (price, qty) = level.parse_decimal().unwrap();
        assert_eq!(price, Decimal::from_str("50000.50").unwrap());
        assert_eq!(qty, Decimal::from_str("1.234").unwrap());
    }

    #[test]
    fn parse_price_level_invalid() {
        let level = PriceLevel("not_a_number".into(), "1.0".into());
        assert!(level.parse_decimal().is_err());
    }

    #[test]
    fn parse_levels_basic() {
        let raw = vec![
            vec!["100.5".into(), "2.0".into()],
            vec!["99.0".into(), "3.5".into()],
        ];
        let levels = parse_levels(&raw).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].price, Decimal::from_str("100.5").unwrap());
        assert_eq!(levels[1].qty, Decimal::from_str("3.5").unwrap());
    }

    #[test]
    fn parse_levels_skips_short_entries() {
        let raw = vec![
            vec!["100.0".into()], // too short — skipped
            vec!["99.0".into(), "3.5".into()],
        ];
        let levels = parse_levels(&raw).unwrap();
        assert_eq!(levels.len(), 1);
    }

    #[test]
    fn parse_levels_empty() {
        let levels = parse_levels(&[]).unwrap();
        assert!(levels.is_empty());
    }

    #[test]
    fn parse_order_book_combines() {
        let bids = vec![vec!["100.0".into(), "1.0".into()]];
        let asks = vec![vec!["101.0".into(), "2.0".into()]];
        let book = parse_order_book(&bids, &asks, 42).unwrap();
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.last_update_id, 42);
    }

    #[test]
    fn book_ticker_ext() {
        let bt = BookTickerMessage {
            event_type: "bookTicker".into(),
            update_id: 1,
            event_time: 0,
            transaction_time: 0,
            symbol: "BTC-USD".into(),
            best_bid_price: "50000.00".into(),
            best_bid_qty: "1.5".into(),
            best_ask_price: "50001.00".into(),
            best_ask_qty: "2.0".into(),
            msg_type: bullet_ws_interface::MessageType::Update,
        };
        let (bp, bq) = bt.best_bid().unwrap();
        let (ap, aq) = bt.best_ask().unwrap();
        assert_eq!(bp, Decimal::from(50000));
        assert_eq!(bq, Decimal::from_str("1.5").unwrap());
        assert_eq!(ap, Decimal::from(50001));
        assert_eq!(aq, Decimal::from(2));
    }

    #[test]
    fn agg_trade_ext() {
        let t = AggTradeMessage {
            event_type: "aggTrade".into(),
            event_time: 0,
            symbol: "BTC-USD".into(),
            agg_trade_id: 1,
            price: "49999.99".into(),
            quantity: "0.001".into(),
            first_trade_id: 0,
            last_trade_id: 0,
            trade_time: 0,
            is_buyer_maker: false,
            tx_hash: String::new(),
            user_address: String::new(),
            order_id: 0,
            is_maker: false,
            is_full_fill: false,
            is_liquidation: false,
            fee: "0".into(),
            net_fee: "0".into(),
            fee_asset: "USD".into(),
            client_order_id: None,
            side: "BUY".into(),
        };
        assert_eq!(t.parsed_price().unwrap(), Decimal::from_str("49999.99").unwrap());
        assert_eq!(t.parsed_quantity().unwrap(), Decimal::from_str("0.001").unwrap());
    }

    #[test]
    fn mark_price_ext() {
        let mp = MarkPriceMessage {
            event_type: "markPriceUpdate".into(),
            event_time: 0,
            symbol: "BTC-USD".into(),
            mark_price: "50000.00".into(),
            index_price: "49999.00".into(),
            estimated_settle_price: None,
            funding_rate: "0.0001".into(),
            next_funding_time: None,
            tx_hash: None,
        };
        assert_eq!(mp.parsed_mark_price().unwrap(), Decimal::from(50000));
        assert_eq!(mp.parsed_index_price().unwrap(), Decimal::from(49999));
        assert_eq!(mp.parsed_funding_rate().unwrap(), Decimal::from_str("0.0001").unwrap());
    }
}
