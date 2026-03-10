pub mod client;
pub mod errors;
pub mod keypair;
pub mod transactions;
pub mod ws;

// Re-export the public surface so consumers can do:
//   import { WasmTradingApi, WasmKeypair, WasmTopic, … } from 'bullet-rust-sdk-wasm'
pub use client::WasmTradingApi;
pub use errors::{WasmError, WasmResult};
pub use keypair::WasmKeypair;
pub use ws::client::WasmWebsocketHandle;
pub use ws::topics::{WasmKlineInterval, WasmOrderbookDepth, WasmTopic};
