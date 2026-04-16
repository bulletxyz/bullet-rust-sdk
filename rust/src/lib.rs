mod client;
mod keypair;
mod metadata;
mod parse;
mod trading;
mod transaction_builder;

pub use trading::{
    ioc_order, limit_order, limit_order_with_id, post_only_order, post_only_order_with_id,
};

/// Error types for the SDK.
pub mod errors;

// Re-export main types at crate root for ergonomic imports
pub use client::{Client, Network};
pub use errors::{SDKError, SDKResult, WSErrors};
pub use generated::types::ApiErrorResponse;
pub use keypair::Keypair;
pub use transaction_builder::{Transaction, UnsignedTransaction};
// Re-export WebSocket close code for pattern matching
pub use reqwest_websocket::CloseCode;
pub use types::CallMessage;

// Re-export WebSocket module and types
pub mod ws;
pub use ws::client::{WebsocketConfig, WebsocketHandle};
#[cfg(not(target_arch = "wasm32"))]
pub use ws::managed::{ManagedWebsocket, ManagedWsConfig, ManagedWsError, WsEvent};
pub use ws::models::ServerMessage;
pub use ws::topics::{KlineInterval, OrderbookDepth, Topic};

/// Re-export the generated Progenitor client and types.
///
/// Use this module to access specific generated types if needed.
/// Most users should just use `Client` which provides access
/// to client methods via `Deref`.
mod generated;
pub mod codegen {
    pub use crate::generated::*;
}

// Re-export generated response type returned by transaction submission.
pub use generated::types::SubmitTxResponse;

// Re-export metadata types for symbol lookups.
pub use metadata::SymbolInfo;

// Re-export typed parsing helpers.
pub use parse::{
    AggTradeExt, BookTickerExt, DepthUpdateExt, MarkPriceExt, ParseDecimal, TypedLevel,
    TypedOrderBook, parse_levels, parse_order_book,
};

// ── On-chain trading types ──────────────────────────────────────────────────
//
// These re-exports let users write `use bullet_rust_sdk::{Side, MarketId, ...}`
// instead of reaching into `bullet_exchange_interface` directly.

/// Order side. `Bid` = buy, `Ask` = sell.
///
/// Follows exchange-internal convention. When integrating:
/// - `Side::Bid` corresponds to a **buy** order
/// - `Side::Ask` corresponds to a **sell** order
pub use bullet_exchange_interface::types::Side;

/// Order execution type.
///
/// - `Limit` — standard limit order
/// - `PostOnly` — maker-only, rejected if it would cross the book
/// - `ImmediateOrCancel` — **use this for market orders** (fill what you can, cancel the rest)
/// - `FillOrKill` — fill entirely or cancel
pub use bullet_exchange_interface::types::OrderType;

/// Numeric market identifier. Wraps a `u16`.
///
/// Resolve a symbol string to a `MarketId` via [`Client::market_id()`]:
/// ```ignore
/// let market_id = client.market_id("BTC-USD").expect("unknown symbol");
/// ```
pub use bullet_exchange_interface::types::MarketId;

/// Exchange-assigned order identifier. Wraps a `u64`.
pub use bullet_exchange_interface::types::OrderId;

/// Client-assigned order identifier. Wraps a `u64`.
pub use bullet_exchange_interface::types::ClientOrderId;

// Order argument structs
pub use bullet_exchange_interface::message::{
    AmendOrderArgs, CancelOrderArgs, NewOrderArgs, NewTriggerOrderArgs, NewTwapOrderArgs,
    PendingTpslPair, Tpsl, TpslPair,
};

/// User action discriminants for schema validation filtering.
pub use bullet_exchange_interface::message::UserActionDiscriminants;

/// A decimal value that must be positive. Wraps `rust_decimal::Decimal`.
///
/// ```ignore
/// use rust_decimal::Decimal;
/// use bullet_rust_sdk::PositiveDecimal;
///
/// let price = PositiveDecimal::try_from(Decimal::from(50000))?;
/// let qty = PositiveDecimal::try_from(Decimal::new(1, 3))?; // 0.001
/// ```
pub use bullet_exchange_interface::decimals::PositiveDecimal;

/// Re-export bullet_rollup types commonly used with the SDK.
pub mod types {
    pub use bullet_exchange_interface;
    pub type CallMessage = bullet_exchange_interface::message::CallMessage<
        bullet_exchange_interface::address::Address,
    >;

    /// User-facing trading action (placing orders, withdrawals, etc.).
    pub type UserAction = bullet_exchange_interface::message::UserAction<
        bullet_exchange_interface::address::Address,
    >;

    /// Permissionless action anyone can call (e.g. `ApplyFunding`).
    pub type PublicAction = bullet_exchange_interface::message::PublicAction<
        bullet_exchange_interface::address::Address,
    >;

    pub use bullet_ws_interface::*;
}

pub use types::{PublicAction, UserAction};
