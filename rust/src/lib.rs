mod client;
mod keypair;
mod memo;
mod metadata;
mod multisig;
mod receipts;
mod sub_account;
mod trading;
mod transaction_builder;
mod vault;

pub use memo::encode_fast_bridge_memo;
pub use sub_account::derive_sub_account_address;
pub use trading::NewOrderExt;
pub use vault::derive_vault_address;

/// Error types for the SDK.
pub mod errors;

pub use bullet_exchange_interface::transaction::{
    Amount as WarpAmount, RuntimeCall, Transaction as SignedTransaction, WarpBytes32 as HexBytes32,
    WarpCall,
};
pub use client::{Client, Network};
pub use errors::{SDKError, SDKResult, WSErrors};
pub use generated::types::ApiErrorResponse;
pub use keypair::Keypair;
pub use multisig::{MAX_MULTISIG_SIGNERS, MultisigConfig, SolanaLedgerMultisigTransaction};
pub use reqwest_websocket::CloseCode;
pub use transaction_builder::{
    SolanaLedgerTransaction, SolanaOffchainTransaction, Transaction, UnsignedTransaction,
};
pub use types::CallMessage;

pub mod ws;
pub use ws::client::{WebsocketConfig, WebsocketHandle};
pub use ws::managed::{ManagedWebsocket, ManagedWsConfig, ManagedWsError, WsEvent};
pub use ws::models::ServerMessage;
pub use ws::topics::{KlineInterval, OrderbookDepth, Topic};

mod generated;
/// Generated REST client and types, also exposed through [`Client`]'s `Deref`.
pub mod codegen {
    pub use crate::generated::*;
}

/// A non-negative decimal value.
pub use bullet_exchange_interface::decimals::PositiveDecimal;
/// User action discriminants for schema validation filtering.
pub use bullet_exchange_interface::message::UserActionDiscriminants;
pub use bullet_exchange_interface::message::{
    AmendOrderArgs, BackstopLiquidatePerpPositionArgsV1, CancelOrderArgs, NewOrderArgs,
    NewTriggerOrderArgs, NewTwapOrderArgs, PendingTpslPair, Tpsl, TpslPair,
};
/// Transaction uniqueness/replay-protection data: `Nonce`, `Generation`, or `Window`.
///
/// Set on a transaction via `UnsignedTransaction::builder().uniqueness(...)` or
/// `Transaction::builder().uniqueness(...)`.
pub use bullet_exchange_interface::transaction::UniquenessData;
/// Client-assigned order identifier. Wraps a `u64`.
pub use bullet_exchange_interface::types::ClientOrderId;
/// Numeric market identifier; resolve symbols with [`Client::market_id`].
pub use bullet_exchange_interface::types::MarketId;
/// Exchange-assigned order identifier. Wraps a `u64`.
pub use bullet_exchange_interface::types::OrderId;
/// Order execution type; immediate-or-cancel orders retain a price limit.
pub use bullet_exchange_interface::types::OrderType;
/// Order side: `Bid` means buy, `Ask` means sell.
pub use bullet_exchange_interface::types::Side;
pub use generated::types::SubmitTxResponse;
pub use metadata::SymbolInfo;

/// Exchange call messages and WebSocket types.
pub mod types {
    pub use bullet_exchange_interface;
    pub type CallMessage = bullet_exchange_interface::message::CallMessage<
        bullet_exchange_interface::address::Address,
    >;

    /// User-facing trading action (placing orders, withdrawals, etc.).
    pub type UserAction =
        bullet_exchange_interface::message::UserAction<bullet_exchange_interface::address::Address>;

    /// Permissionless action anyone can call (e.g. `ApplyFunding`).
    pub type PublicAction = bullet_exchange_interface::message::PublicAction<
        bullet_exchange_interface::address::Address,
    >;

    pub use bullet_ws_interface::*;
}

pub use types::{PublicAction, UserAction};
