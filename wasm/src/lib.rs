use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

pub mod client;
pub mod errors;
pub mod generated;
pub mod keypair;
pub mod metadata;
pub mod multisig;
pub mod sub_account;
pub mod transaction_builder;
pub mod utils;
pub mod vault;
pub mod ws;

// Re-export the public surface so consumers can do:
//   import { WasmTradingApi, WasmKeypair, WasmTopic, … } from 'bullet-rust-sdk-wasm'
pub use client::*;
pub use errors::*;
pub use generated::*;
pub use keypair::*;
pub use metadata::*;
pub use multisig::*;
pub use sub_account::*;
pub use transaction_builder::*;
pub use utils::decimal::WasmDecimal;
pub use vault::*;
pub use ws::client::*;
pub use ws::topics::*;
