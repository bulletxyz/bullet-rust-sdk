//! Shared infrastructure and cross-cutting types.
//!
//! - `to_json` helper used by every submodule
//! - `ModuleRef`, `RateParams`, `RateLimit`, `ChainInfo`, `RollupConstants`

use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ──────────────────────────────────────────────────────────────────────────────
// Helper: serialise inner value to a JSON string for the JS side.
// ──────────────────────────────────────────────────────────────────────────────
pub(super) fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ══════════════════════════════════════════════════════════════════════════════
// ModuleRef
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = ModuleRef)]
pub struct WasmModuleRef(pub(crate) sdk::ModuleRef);

#[wasm_bindgen(js_class = ModuleRef)]
impl WasmModuleRef {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.0.name.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RateParams
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = RateParams)]
pub struct WasmRateParams(pub(crate) sdk::RateParams);

#[wasm_bindgen(js_class = RateParams)]
impl WasmRateParams {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = maxBorrowRate)]
    pub fn max_borrow_rate(&self) -> String {
        self.0.max_borrow_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = minBorrowRate)]
    pub fn min_borrow_rate(&self) -> String {
        self.0.min_borrow_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = optimalBorrowRate)]
    pub fn optimal_borrow_rate(&self) -> String {
        self.0.optimal_borrow_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = optimalUtilisationRate)]
    pub fn optimal_utilisation_rate(&self) -> String {
        self.0.optimal_utilisation_rate.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RateLimit
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = RateLimit)]
pub struct WasmRateLimit(pub(crate) sdk::RateLimit);

#[wasm_bindgen(js_class = RateLimit)]
impl WasmRateLimit {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn interval(&self) -> String {
        self.0.interval.clone()
    }

    #[wasm_bindgen(getter, js_name = intervalNum)]
    pub fn interval_num(&self) -> i32 {
        self.0.interval_num
    }

    #[wasm_bindgen(getter)]
    pub fn limit(&self) -> i32 {
        self.0.limit
    }

    #[wasm_bindgen(getter, js_name = rateLimitType)]
    pub fn rate_limit_type(&self) -> String {
        self.0.rate_limit_type.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ChainInfo
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = ChainInfo)]
pub struct WasmChainInfo(pub(crate) sdk::ChainInfo);

#[wasm_bindgen(js_class = ChainInfo)]
impl WasmChainInfo {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = addressPrefix)]
    pub fn address_prefix(&self) -> String {
        self.0.address_prefix.clone()
    }

    #[wasm_bindgen(getter, js_name = chainId)]
    pub fn chain_id(&self) -> f64 {
        self.0.chain_id as f64
    }

    #[wasm_bindgen(getter, js_name = chainName)]
    pub fn chain_name(&self) -> String {
        self.0.chain_name.clone()
    }

    #[wasm_bindgen(getter, js_name = gasTokenId)]
    pub fn gas_token_id(&self) -> String {
        self.0.gas_token_id.clone()
    }

    #[wasm_bindgen(getter, js_name = hyperlaneDomain)]
    pub fn hyperlane_domain(&self) -> f64 {
        self.0.hyperlane_domain as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RollupConstants
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = RollupConstants)]
pub struct WasmRollupConstants(pub(crate) sdk::RollupConstants);

#[wasm_bindgen(js_class = RollupConstants)]
impl WasmRollupConstants {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = addressPrefix)]
    pub fn address_prefix(&self) -> String {
        self.0.address_prefix.clone()
    }

    #[wasm_bindgen(getter, js_name = chainId)]
    pub fn chain_id(&self) -> f64 {
        self.0.chain_id as f64
    }

    #[wasm_bindgen(getter, js_name = chainName)]
    pub fn chain_name(&self) -> String {
        self.0.chain_name.clone()
    }

    #[wasm_bindgen(getter, js_name = gasTokenId)]
    pub fn gas_token_id(&self) -> String {
        self.0.gas_token_id.clone()
    }

    #[wasm_bindgen(getter, js_name = hyperlaneDomain)]
    pub fn hyperlane_domain(&self) -> f64 {
        self.0.hyperlane_domain as f64
    }
}
