//! WASM wrappers for exchange metadata types.
//!
//! Mirrors `bullet_rust_sdk::SymbolInfo` so JS/TS consumers get typed
//! access to symbol lookups instead of parsing JSON strings.

use bullet_rust_sdk::SymbolInfo;
use wasm_bindgen::prelude::*;

/// Symbol information cached from the exchange.
#[wasm_bindgen(js_name = SymbolInfo)]
pub struct WasmSymbolInfo(pub(crate) SymbolInfo);

#[wasm_bindgen(js_class = SymbolInfo)]
impl WasmSymbolInfo {
    /// Trading pair symbol (e.g. `"BTC-USD"`).
    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    /// Numeric market identifier.
    #[wasm_bindgen(getter, js_name = marketId)]
    pub fn market_id(&self) -> u16 {
        self.0.market_id.0
    }

    /// Trading status (e.g. `"TRADING"`, `"HALT"`).
    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.0.status.clone()
    }

    /// Base asset (e.g. `"BTC"`).
    #[wasm_bindgen(getter, js_name = baseAsset)]
    pub fn base_asset(&self) -> String {
        self.0.base_asset.clone()
    }

    /// Quote asset (e.g. `"USD"`).
    #[wasm_bindgen(getter, js_name = quoteAsset)]
    pub fn quote_asset(&self) -> String {
        self.0.quote_asset.clone()
    }

    /// Price decimal precision.
    #[wasm_bindgen(getter, js_name = pricePrecision)]
    pub fn price_precision(&self) -> u8 {
        self.0.price_precision
    }

    /// Quantity decimal precision.
    #[wasm_bindgen(getter, js_name = quantityPrecision)]
    pub fn quantity_precision(&self) -> u8 {
        self.0.quantity_precision
    }
}
