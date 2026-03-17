//! Borrow/lend and insurance types.
//!
//! Types: `BorrowLendPoolResponse`, `InsuranceAsset`, `InsuranceBalance`

use super::common::to_json;
use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ══════════════════════════════════════════════════════════════════════════════
// BorrowLendPoolResponse
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = BorrowLendPoolResponse)]
pub struct WasmBorrowLendPoolResponse(pub(crate) sdk::BorrowLendPoolResponse);

#[wasm_bindgen(js_class = BorrowLendPoolResponse)]
impl WasmBorrowLendPoolResponse {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = accumulatedProtocolFees)]
    pub fn accumulated_protocol_fees(&self) -> String {
        self.0.accumulated_protocol_fees.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn asset(&self) -> String {
        self.0.asset.clone()
    }

    #[wasm_bindgen(getter, js_name = assetId)]
    pub fn asset_id(&self) -> i32 {
        self.0.asset_id.into()
    }

    #[wasm_bindgen(getter, js_name = availableAmount)]
    pub fn available_amount(&self) -> String {
        self.0.available_amount.clone()
    }

    #[wasm_bindgen(getter, js_name = borrowLimit)]
    pub fn borrow_limit(&self) -> String {
        self.0.borrow_limit.clone()
    }

    #[wasm_bindgen(getter, js_name = borrowedAmount)]
    pub fn borrowed_amount(&self) -> String {
        self.0.borrowed_amount.clone()
    }

    #[wasm_bindgen(getter, js_name = cumulativeBorrowRate)]
    pub fn cumulative_borrow_rate(&self) -> String {
        self.0.cumulative_borrow_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = cumulativeDepositRate)]
    pub fn cumulative_deposit_rate(&self) -> String {
        self.0.cumulative_deposit_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = depositLimit)]
    pub fn deposit_limit(&self) -> String {
        self.0.deposit_limit.clone()
    }

    #[wasm_bindgen(getter, js_name = interestFeeTenthBps)]
    pub fn interest_fee_tenth_bps(&self) -> f64 {
        self.0.interest_fee_tenth_bps as f64
    }

    #[wasm_bindgen(getter, js_name = isActive)]
    pub fn is_active(&self) -> bool {
        self.0.is_active
    }

    #[wasm_bindgen(getter, js_name = lastUpdateTimestamp)]
    pub fn last_update_timestamp(&self) -> f64 {
        self.0.last_update_timestamp as f64
    }

    #[wasm_bindgen(getter, js_name = unclaimedProtocolFees)]
    pub fn unclaimed_protocol_fees(&self) -> String {
        self.0.unclaimed_protocol_fees.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// InsuranceAsset
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = InsuranceAsset)]
pub struct WasmInsuranceAsset(pub(crate) sdk::InsuranceAsset);

#[wasm_bindgen(js_class = InsuranceAsset)]
impl WasmInsuranceAsset {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn asset(&self) -> String {
        self.0.asset.clone()
    }

    #[wasm_bindgen(getter, js_name = marginBalance)]
    pub fn margin_balance(&self) -> String {
        self.0.margin_balance.clone()
    }

    #[wasm_bindgen(getter, js_name = updateTime)]
    pub fn update_time(&self) -> f64 {
        self.0.update_time as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// InsuranceBalance
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = InsuranceBalance)]
pub struct WasmInsuranceBalance(pub(crate) sdk::InsuranceBalance);

#[wasm_bindgen(js_class = InsuranceBalance)]
impl WasmInsuranceBalance {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }
}
