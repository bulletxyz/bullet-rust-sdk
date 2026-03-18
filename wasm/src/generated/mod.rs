//! Wasm-bindgen wrappers for all generated REST response types and endpoints,
//! organised by domain.
//!
//! | Module      | Contents                                                               |
//! |-------------|------------------------------------------------------------------------|
//! | `client`    | `impl WasmTradingApi` — all progenitor REST endpoint wrappers          |
//! | `account`   | `Account`, `AccountAsset`, `AccountPosition`, `Balance`               |
//! | `borrow`    | `BorrowLendPoolResponse`, `InsuranceAsset`, `InsuranceBalance`         |
//! | `common`    | `ChainInfo`, `ModuleRef`, `RateLimit`, `RateParams`, `RollupConstants` |
 //! | `market`    | `Asset`, `ExchangeInfo`, `FundingRate`, `OrderBook`, `PingResponse`,  |
 //! |             | `PriceTicker`, `TradingSymbol`, `Ticker24hr`, `TimeResponse`, `Trade`  |
//! | `orders`    | `BinanceOrder`, `Bracket`, `LeverageBracket`                          |
//! | `tx`        | `LedgerEvent`, `SubmitTxRequest`, `SubmitTxResponse`,                 |
//! |             | `TxReceipt`, `TxResult`, `TxStatus`                                    |

pub mod account;
pub mod borrow;
pub mod client;
pub mod common;
pub mod market;
pub mod orders;
pub mod tx;

// Flatten everything into the generated namespace so callers can write:
//   use crate::generated::WasmAccount;
// rather than:
//   use crate::generated::account::WasmAccount;

pub use account::{WasmAccount, WasmAccountAsset, WasmAccountPosition, WasmBalance};
pub use borrow::{WasmBorrowLendPoolResponse, WasmInsuranceAsset, WasmInsuranceBalance};
pub use common::{
    WasmChainInfo, WasmModuleRef, WasmRateLimit, WasmRateParams, WasmRollupConstants,
};
pub use market::{
    WasmAsset, WasmExchangeInfo, WasmFundingRate, WasmOrderBook, WasmPingResponse, WasmPriceTicker,
    WasmSymbol, WasmTicker24hr, WasmTimeResponse, WasmTrade,
};
pub use orders::{WasmBinanceOrder, WasmBracket, WasmLeverageBracket};
pub use tx::{
    WasmLedgerEvent, WasmSubmitTxRequest, WasmSubmitTxResponse, WasmTxReceipt, WasmTxResult,
    WasmTxStatus,
};
