use bullet_rust_sdk::TradingApi;
use wasm_bindgen::prelude::*;

use crate::errors::WasmResult;

/// Full Bullet trading API client (REST + WebSocket).
///
/// All REST responses are returned as JSON strings.
/// Errors are thrown as JavaScript `Error` objects with a `.message` property.
#[wasm_bindgen(js_name = TradingApi)]
pub struct WasmTradingApi {
    pub(crate) inner: TradingApi,
}

#[wasm_bindgen(js_class = TradingApi)]
impl WasmTradingApi {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Connect to the mainnet REST endpoint and validate the remote schema.
    pub async fn mainnet() -> WasmResult<WasmTradingApi> {
        Ok(WasmTradingApi { inner: TradingApi::mainnet().await? })
    }

    /// Connect to a custom REST endpoint URL.
    pub async fn connect(url: &str) -> WasmResult<WasmTradingApi> {
        Ok(WasmTradingApi { inner: TradingApi::new(url, None).await? })
    }

    // ── Metadata ──────────────────────────────────────────────────────────────

    /// Chain ID for the connected network.
    #[wasm_bindgen(js_name = chainId)]
    pub fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    /// Chain hash as a lowercase hex string.
    #[wasm_bindgen(js_name = chainHash)]
    pub fn chain_hash(&self) -> String {
        hex::encode(self.inner.chain_hash())
    }

    /// REST API base URL.
    pub fn url(&self) -> String {
        self.inner.url().to_string()
    }

    /// WebSocket URL.
    #[wasm_bindgen(js_name = wsUrl)]
    pub fn ws_url(&self) -> String {
        self.inner.ws_url().to_string()
    }

}
