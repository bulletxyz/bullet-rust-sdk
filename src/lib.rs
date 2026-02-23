mod client;
mod keypair;
mod transactions;

/// Error types for the SDK.
pub mod errors;

// Re-export main types at crate root for ergonomic imports
pub use client::{MAINNET_URL, TradingApi};
pub use errors::{SDKError, SDKResult, WSErrors};
pub use keypair::Keypair;
// Re-export WebSocket close code for pattern matching
pub use reqwest_websocket::CloseCode;
pub use types::CallMessage;

// Re-export WebSocket module and types
pub mod ws;
pub use ws::client::{WebsocketConfig, WebsocketHandle};
pub use ws::models::ServerMessage;
pub use ws::topics::{KlineInterval, OrderbookDepth, Topic};

/// Re-export the generated Progenitor client and types.
///
/// Use this module to access specific generated types if needed.
/// Most users should just use `TradingApi` which provides access
/// to client methods via `Deref`.
mod generated;
pub mod codegen {
    pub use crate::generated::*;
}

/// Re-export bullet_rollup types commonly used with the SDK.
pub mod types {
    pub use bullet_exchange_interface::address::Address;
    pub use bullet_exchange_interface::transaction::{Transaction, UnsignedTransaction};

    /// CallMessage type alias with the Address type pre-filled.
    pub type CallMessage = bullet_exchange_interface::message::CallMessage<Address>;
    pub use bullet_ws_interface::*;
}
