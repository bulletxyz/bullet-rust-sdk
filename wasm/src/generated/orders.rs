//! Order types.
//!
//! Types: `BinanceOrder`, `LeverageBracket`, `Bracket`

use super::common::to_json;
use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ══════════════════════════════════════════════════════════════════════════════
// BinanceOrder
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = BinanceOrder)]
pub struct WasmBinanceOrder(pub(crate) sdk::BinanceOrder);

#[wasm_bindgen(js_class = BinanceOrder)]
impl WasmBinanceOrder {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = avgPrice)]
    pub fn avg_price(&self) -> String {
        self.0.avg_price.clone()
    }

    #[wasm_bindgen(getter, js_name = clientOrderId)]
    pub fn client_order_id(&self) -> Option<String> {
        self.0.client_order_id.clone()
    }

    #[wasm_bindgen(getter, js_name = closePosition)]
    pub fn close_position(&self) -> bool {
        self.0.close_position
    }

    #[wasm_bindgen(getter, js_name = cumQty)]
    pub fn cum_qty(&self) -> String {
        self.0.cum_qty.clone()
    }

    #[wasm_bindgen(getter, js_name = cumQuote)]
    pub fn cum_quote(&self) -> String {
        self.0.cum_quote.clone()
    }

    #[wasm_bindgen(getter, js_name = executedQty)]
    pub fn executed_qty(&self) -> String {
        self.0.executed_qty.clone()
    }

    #[wasm_bindgen(getter, js_name = goodTillDate)]
    pub fn good_till_date(&self) -> f64 {
        self.0.good_till_date as f64
    }

    #[wasm_bindgen(getter, js_name = orderId)]
    pub fn order_id(&self) -> f64 {
        self.0.order_id as f64
    }

    #[wasm_bindgen(getter, js_name = orderType)]
    pub fn order_type(&self) -> String {
        self.0.order_type.clone()
    }

    #[wasm_bindgen(getter, js_name = origQty)]
    pub fn orig_qty(&self) -> String {
        self.0.orig_qty.clone()
    }

    #[wasm_bindgen(getter, js_name = origType)]
    pub fn orig_type(&self) -> String {
        self.0.orig_type.clone()
    }

    #[wasm_bindgen(getter, js_name = positionSide)]
    pub fn position_side(&self) -> String {
        self.0.position_side.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn price(&self) -> String {
        self.0.price.clone()
    }

    #[wasm_bindgen(getter, js_name = priceMatch)]
    pub fn price_match(&self) -> String {
        self.0.price_match.clone()
    }

    #[wasm_bindgen(getter, js_name = priceProtect)]
    pub fn price_protect(&self) -> bool {
        self.0.price_protect
    }

    #[wasm_bindgen(getter, js_name = reduceOnly)]
    pub fn reduce_only(&self) -> bool {
        self.0.reduce_only
    }

    #[wasm_bindgen(getter, js_name = selfTradePreventionMode)]
    pub fn self_trade_prevention_mode(&self) -> String {
        self.0.self_trade_prevention_mode.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn side(&self) -> String {
        self.0.side.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.0.status.clone()
    }

    #[wasm_bindgen(getter, js_name = stopPrice)]
    pub fn stop_price(&self) -> Option<String> {
        self.0.stop_price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    #[wasm_bindgen(getter, js_name = timeInForce)]
    pub fn time_in_force(&self) -> String {
        self.0.time_in_force.clone()
    }

    #[wasm_bindgen(getter, js_name = updateTime)]
    pub fn update_time(&self) -> f64 {
        self.0.update_time as f64
    }

    #[wasm_bindgen(getter, js_name = workingType)]
    pub fn working_type(&self) -> String {
        self.0.working_type.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Bracket
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Bracket)]
pub struct WasmBracket(pub(crate) sdk::Bracket);

#[wasm_bindgen(js_class = Bracket)]
impl WasmBracket {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn bracket(&self) -> i32 {
        self.0.bracket
    }

    #[wasm_bindgen(getter)]
    pub fn cum(&self) -> f64 {
        self.0.cum
    }

    #[wasm_bindgen(getter, js_name = initialLeverage)]
    pub fn initial_leverage(&self) -> i32 {
        self.0.initial_leverage
    }

    #[wasm_bindgen(getter, js_name = maintMarginRatio)]
    pub fn maint_margin_ratio(&self) -> f64 {
        self.0.maint_margin_ratio
    }

    #[wasm_bindgen(getter, js_name = notionalCap)]
    pub fn notional_cap(&self) -> f64 {
        self.0.notional_cap
    }

    #[wasm_bindgen(getter, js_name = notionalFloor)]
    pub fn notional_floor(&self) -> f64 {
        self.0.notional_floor
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// LeverageBracket
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = LeverageBracket)]
pub struct WasmLeverageBracket(pub(crate) sdk::LeverageBracket);

#[wasm_bindgen(js_class = LeverageBracket)]
impl WasmLeverageBracket {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = notionalCoef)]
    pub fn notional_coef(&self) -> Option<f64> {
        self.0.notional_coef
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }
}
