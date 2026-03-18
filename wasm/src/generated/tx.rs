//! Transaction types.
//!
//! Types: `SubmitTxRequest`, `SubmitTxResponse`, `TxReceipt`,
//!        `TxResult` (enum), `TxStatus` (enum), `LedgerEvent`

use super::common::to_json;
use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ══════════════════════════════════════════════════════════════════════════════
// TxResult (enum)
// ══════════════════════════════════════════════════════════════════════════════

/// Transaction execution result.
#[wasm_bindgen(js_name = TxResult)]
pub enum WasmTxResult {
    Successful,
    Reverted,
    Skipped,
}

impl From<sdk::TxResult> for WasmTxResult {
    fn from(r: sdk::TxResult) -> Self {
        match r {
            sdk::TxResult::Successful => WasmTxResult::Successful,
            sdk::TxResult::Reverted => WasmTxResult::Reverted,
            sdk::TxResult::Skipped => WasmTxResult::Skipped,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TxStatus (enum)
// ══════════════════════════════════════════════════════════════════════════════

/// Transaction lifecycle status.
#[wasm_bindgen(js_name = TxStatus)]
pub enum WasmTxStatus {
    Unknown,
    Dropped,
    Submitted,
    Published,
    Processed,
    Finalized,
}

impl From<sdk::TxStatus> for WasmTxStatus {
    fn from(s: sdk::TxStatus) -> Self {
        match s {
            sdk::TxStatus::Unknown => WasmTxStatus::Unknown,
            sdk::TxStatus::Dropped => WasmTxStatus::Dropped,
            sdk::TxStatus::Submitted => WasmTxStatus::Submitted,
            sdk::TxStatus::Published => WasmTxStatus::Published,
            sdk::TxStatus::Processed => WasmTxStatus::Processed,
            sdk::TxStatus::Finalized => WasmTxStatus::Finalized,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// LedgerEvent
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = LedgerEvent)]
pub struct WasmLedgerEvent(pub(crate) sdk::LedgerEvent);

#[wasm_bindgen(js_class = LedgerEvent)]
impl WasmLedgerEvent {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.0.key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn number(&self) -> f64 {
        self.0.number as f64
    }

    #[wasm_bindgen(getter, js_name = txHash)]
    pub fn tx_hash(&self) -> Option<String> {
        self.0.tx_hash.clone()
    }

    /// The event `type` field (renamed from `type_` in Rust to avoid keyword).
    #[wasm_bindgen(getter, js_name = type)]
    pub fn event_type(&self) -> String {
        self.0.type_.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TxReceipt
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = TxReceipt)]
pub struct WasmTxReceipt(pub(crate) sdk::TxReceipt);

#[wasm_bindgen(js_class = TxReceipt)]
impl WasmTxReceipt {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    /// Returns the result as a string: `"successful"`, `"reverted"`, or `"skipped"`.
    #[wasm_bindgen(getter)]
    pub fn result(&self) -> String {
        self.0.result.to_string()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// SubmitTxRequest
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = SubmitTxRequest)]
pub struct WasmSubmitTxRequest(pub(crate) sdk::SubmitTxRequest);

#[wasm_bindgen(js_class = SubmitTxRequest)]
impl WasmSubmitTxRequest {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn body(&self) -> String {
        self.0.body.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// SubmitTxResponse
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = SubmitTxResponse)]
pub struct WasmSubmitTxResponse(pub(crate) sdk::SubmitTxResponse);

#[wasm_bindgen(js_class = SubmitTxResponse)]
impl WasmSubmitTxResponse {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    /// The transaction hash.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.0.id.clone()
    }

    /// Transaction number (if processed).
    #[wasm_bindgen(getter, js_name = txNumber)]
    pub fn tx_number(&self) -> Option<f64> {
        self.0.tx_number.map(|n| n as f64)
    }

    /// The current status as a string.
    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.0.status.to_string()
    }
}
