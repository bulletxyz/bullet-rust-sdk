pub mod client;
pub mod errors;
pub mod generated;
pub mod keypair;
pub mod transactions;
pub mod ws;

// Re-export the public surface so consumers can do:
//   import { WasmTradingApi, WasmKeypair, WasmTopic, … } from 'bullet-rust-sdk-wasm'
pub use client::WasmTradingApi;
pub use errors::{WasmError, WasmResult};
pub use generated::{
    WasmAccount, WasmAccountAsset, WasmAccountPosition, WasmAsset, WasmBalance,
    WasmBinanceOrder, WasmBorrowLendPoolResponse, WasmBracket, WasmChainInfo, WasmExchangeInfo,
    WasmFundingRate, WasmInsuranceAsset, WasmInsuranceBalance, WasmLedgerEvent,
    WasmLeverageBracket, WasmModuleRef, WasmOrderBook, WasmPingResponse, WasmPriceTicker,
    WasmRateLimit, WasmRateParams, WasmRollupConstants, WasmSubmitTxRequest,
    WasmSubmitTxResponse, WasmSymbol, WasmTicker24hr, WasmTimeResponse, WasmTrade,
    WasmTxReceipt, WasmTxResult, WasmTxStatus,
};
pub use keypair::WasmKeypair;
pub use ws::client::WasmWebsocketHandle;
pub use ws::topics::{WasmKlineInterval, WasmOrderbookDepth, WasmTopic};
