//! Account and balance types.
//!
//! Types: `Account`, `AccountAsset`, `AccountPosition`, `Balance`

use super::common::to_json;
use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ══════════════════════════════════════════════════════════════════════════════
// Account
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Account)]
pub struct WasmAccount(pub(crate) sdk::Account);

#[wasm_bindgen(js_class = Account)]
impl WasmAccount {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = availableBalance)]
    pub fn available_balance(&self) -> String {
        self.0.available_balance.clone()
    }

    #[wasm_bindgen(getter, js_name = maxWithdrawAmount)]
    pub fn max_withdraw_amount(&self) -> String {
        self.0.max_withdraw_amount.clone()
    }

    #[wasm_bindgen(getter, js_name = totalCrossUnPnl)]
    pub fn total_cross_un_pnl(&self) -> String {
        self.0.total_cross_un_pnl.clone()
    }

    #[wasm_bindgen(getter, js_name = totalCrossWalletBalance)]
    pub fn total_cross_wallet_balance(&self) -> String {
        self.0.total_cross_wallet_balance.clone()
    }

    #[wasm_bindgen(getter, js_name = totalInitialMargin)]
    pub fn total_initial_margin(&self) -> String {
        self.0.total_initial_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = totalMaintMargin)]
    pub fn total_maint_margin(&self) -> String {
        self.0.total_maint_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = totalMarginBalance)]
    pub fn total_margin_balance(&self) -> String {
        self.0.total_margin_balance.clone()
    }

    #[wasm_bindgen(getter, js_name = totalOpenOrderInitialMargin)]
    pub fn total_open_order_initial_margin(&self) -> String {
        self.0.total_open_order_initial_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = totalPositionInitialMargin)]
    pub fn total_position_initial_margin(&self) -> String {
        self.0.total_position_initial_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = totalUnrealizedProfit)]
    pub fn total_unrealized_profit(&self) -> String {
        self.0.total_unrealized_profit.clone()
    }

    #[wasm_bindgen(getter, js_name = totalWalletBalance)]
    pub fn total_wallet_balance(&self) -> String {
        self.0.total_wallet_balance.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// AccountAsset
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = AccountAsset)]
pub struct WasmAccountAsset(pub(crate) sdk::AccountAsset);

#[wasm_bindgen(js_class = AccountAsset)]
impl WasmAccountAsset {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn asset(&self) -> String {
        self.0.asset.clone()
    }

    #[wasm_bindgen(getter, js_name = assetId)]
    pub fn asset_id(&self) -> i32 {
        self.0.asset_id.into()
    }

    #[wasm_bindgen(getter, js_name = updateTime)]
    pub fn update_time(&self) -> f64 {
        self.0.update_time as f64
    }

    #[wasm_bindgen(getter, js_name = walletBalance)]
    pub fn wallet_balance(&self) -> String {
        self.0.wallet_balance.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// AccountPosition
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = AccountPosition)]
pub struct WasmAccountPosition(pub(crate) sdk::AccountPosition);

#[wasm_bindgen(js_class = AccountPosition)]
impl WasmAccountPosition {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = entryPrice)]
    pub fn entry_price(&self) -> String {
        self.0.entry_price.clone()
    }

    #[wasm_bindgen(getter, js_name = initialMargin)]
    pub fn initial_margin(&self) -> String {
        self.0.initial_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = maintMargin)]
    pub fn maint_margin(&self) -> String {
        self.0.maint_margin.clone()
    }

    #[wasm_bindgen(getter, js_name = marketId)]
    pub fn market_id(&self) -> i32 {
        self.0.market_id.into()
    }

    #[wasm_bindgen(getter, js_name = positionAmt)]
    pub fn position_amt(&self) -> String {
        self.0.position_amt.clone()
    }

    #[wasm_bindgen(getter, js_name = positionSide)]
    pub fn position_side(&self) -> String {
        self.0.position_side.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    #[wasm_bindgen(getter, js_name = unrealizedProfit)]
    pub fn unrealized_profit(&self) -> String {
        self.0.unrealized_profit.clone()
    }

    #[wasm_bindgen(getter, js_name = updateTime)]
    pub fn update_time(&self) -> f64 {
        self.0.update_time as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Balance
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Balance)]
pub struct WasmBalance(pub(crate) sdk::Balance);

#[wasm_bindgen(js_class = Balance)]
impl WasmBalance {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = accountAlias)]
    pub fn account_alias(&self) -> String {
        self.0.account_alias.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn asset(&self) -> String {
        self.0.asset.clone()
    }

    #[wasm_bindgen(getter, js_name = availableBalance)]
    pub fn available_balance(&self) -> String {
        self.0.available_balance.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn balance(&self) -> String {
        self.0.balance.clone()
    }

    #[wasm_bindgen(getter, js_name = crossUnPnl)]
    pub fn cross_un_pnl(&self) -> String {
        self.0.cross_un_pnl.clone()
    }

    #[wasm_bindgen(getter, js_name = crossWalletBalance)]
    pub fn cross_wallet_balance(&self) -> String {
        self.0.cross_wallet_balance.clone()
    }

    #[wasm_bindgen(getter, js_name = marginAvailable)]
    pub fn margin_available(&self) -> bool {
        self.0.margin_available
    }

    #[wasm_bindgen(getter, js_name = maxWithdrawAmount)]
    pub fn max_withdraw_amount(&self) -> String {
        self.0.max_withdraw_amount.clone()
    }

    #[wasm_bindgen(getter, js_name = updateTime)]
    pub fn update_time(&self) -> f64 {
        self.0.update_time as f64
    }
}
