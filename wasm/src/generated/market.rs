//! Market data types.
//!
//! Types: `ExchangeInfo`, `Asset`, `Symbol`, `PriceTicker`, `Ticker24hr`,
//!        `OrderBook`, `FundingRate`, `Trade`, `TimeResponse`, `PingResponse`

use super::common::{to_json, WasmChainInfo, WasmRateLimit};
use bullet_rust_sdk::codegen::types as sdk;
use wasm_bindgen::prelude::*;

// ══════════════════════════════════════════════════════════════════════════════
// Asset
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Asset)]
pub struct WasmAsset(pub(crate) sdk::Asset);

#[wasm_bindgen(js_class = Asset)]
impl WasmAsset {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn asset(&self) -> String {
        self.0.asset.clone()
    }

    // TODO: should be u16 - the Trading API uses u16 internally but utoipa emits
    // `format: int32` for unsigned types. Once the Trading API PR adds
    // `#[schema(format = "uint16")]` and the codegen PR is merged, this will be
    // generated correctly as u16.
    #[wasm_bindgen(getter, js_name = assetId)]
    pub fn asset_id(&self) -> i32 {
        self.0.asset_id
    }

    // TODO: should be u8 - same codegen issue as asset_id above. Will be fixed
    // once the Trading API PR adds `#[schema(format = "uint8")]`.
    #[wasm_bindgen(getter)]
    pub fn decimals(&self) -> i32 {
        self.0.decimals
    }

    #[wasm_bindgen(getter, js_name = marginAvailable)]
    pub fn margin_available(&self) -> bool {
        self.0.margin_available
    }

    #[wasm_bindgen(getter, js_name = tokenId)]
    pub fn token_id(&self) -> Option<String> {
        self.0.token_id.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ExchangeInfo
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = ExchangeInfo)]
pub struct WasmExchangeInfo(pub(crate) sdk::ExchangeInfo);

#[wasm_bindgen(js_class = ExchangeInfo)]
impl WasmExchangeInfo {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = chainHash)]
    pub fn chain_hash(&self) -> Option<String> {
        self.0.chain_hash.clone()
    }

    #[wasm_bindgen(getter, js_name = chainInfo)]
    pub fn chain_info(&self) -> Option<WasmChainInfo> {
        self.0.chain_info.clone().map(WasmChainInfo)
    }

    #[wasm_bindgen(getter)]
    pub fn assets(&self) -> Vec<WasmAsset> {
        self.0.assets.iter().cloned().map(WasmAsset).collect()
    }

    #[wasm_bindgen(getter)]
    pub fn symbols(&self) -> Vec<WasmSymbol> {
        self.0.symbols.iter().cloned().map(WasmSymbol).collect()
    }

    #[wasm_bindgen(getter, js_name = rateLimits)]
    pub fn rate_limits(&self) -> Vec<WasmRateLimit> {
        self.0
            .rate_limits
            .iter()
            .cloned()
            .map(WasmRateLimit)
            .collect()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// FundingRate
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = FundingRate)]
pub struct WasmFundingRate(pub(crate) sdk::FundingRate);

#[wasm_bindgen(js_class = FundingRate)]
impl WasmFundingRate {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = fundingRate)]
    pub fn funding_rate(&self) -> String {
        self.0.funding_rate.clone()
    }

    #[wasm_bindgen(getter, js_name = fundingTime)]
    pub fn funding_time(&self) -> f64 {
        self.0.funding_time as f64
    }

    #[wasm_bindgen(getter, js_name = markPrice)]
    pub fn mark_price(&self) -> String {
        self.0.mark_price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// OrderBook
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = OrderBook)]
pub struct WasmOrderBook(pub(crate) sdk::OrderBook);

#[wasm_bindgen(js_class = OrderBook)]
impl WasmOrderBook {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    /// Message event time.
    #[wasm_bindgen(getter, js_name = E)]
    pub fn e(&self) -> f64 {
        self.0.e as f64
    }

    /// Transaction time.
    #[wasm_bindgen(getter, js_name = T)]
    pub fn t(&self) -> f64 {
        self.0.t as f64
    }

    #[wasm_bindgen(getter, js_name = lastUpdateId)]
    pub fn last_update_id(&self) -> f64 {
        self.0.last_update_id as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PingResponse
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = PingResponse)]
pub struct WasmPingResponse(pub(crate) sdk::PingResponse);

#[wasm_bindgen(js_class = PingResponse)]
impl WasmPingResponse {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PriceTicker
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = PriceTicker)]
pub struct WasmPriceTicker(pub(crate) sdk::PriceTicker);

#[wasm_bindgen(js_class = PriceTicker)]
impl WasmPriceTicker {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn price(&self) -> String {
        self.0.price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn time(&self) -> f64 {
        self.0.time as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Symbol
// ══════════════════════════════════════════════════════════════════════════════

// Named `TradingSymbol` (not `Symbol`) to avoid shadowing the JS built-in
// `Symbol` global. wasm-bindgen emits `class Symbol { … }` at the top-level
// of the generated CJS file, which creates a temporal dead zone that breaks
// every `if (Symbol.dispose) …` guard that appears before the class
// declaration in the same file.
#[wasm_bindgen(js_name = TradingSymbol)]
pub struct WasmSymbol(pub(crate) sdk::Symbol);

#[wasm_bindgen(js_class = TradingSymbol)]
impl WasmSymbol {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    #[wasm_bindgen(getter, js_name = baseAsset)]
    pub fn base_asset(&self) -> String {
        self.0.base_asset.clone()
    }

    #[wasm_bindgen(getter, js_name = quoteAsset)]
    pub fn quote_asset(&self) -> String {
        self.0.quote_asset.clone()
    }

    #[wasm_bindgen(getter, js_name = marginAsset)]
    pub fn margin_asset(&self) -> String {
        self.0.margin_asset.clone()
    }

    #[wasm_bindgen(getter, js_name = marketId)]
    pub fn market_id(&self) -> i32 {
        self.0.market_id
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.0.status.clone()
    }

    #[wasm_bindgen(getter, js_name = pricePrecision)]
    pub fn price_precision(&self) -> i32 {
        self.0.price_precision
    }

    #[wasm_bindgen(getter, js_name = quantityPrecision)]
    pub fn quantity_precision(&self) -> i32 {
        self.0.quantity_precision
    }

    #[wasm_bindgen(getter, js_name = contractType)]
    pub fn contract_type(&self) -> String {
        self.0.contract_type.clone()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Ticker24hr
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Ticker24hr)]
pub struct WasmTicker24hr(pub(crate) sdk::Ticker24hr);

#[wasm_bindgen(js_class = Ticker24hr)]
impl WasmTicker24hr {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn symbol(&self) -> String {
        self.0.symbol.clone()
    }

    #[wasm_bindgen(getter, js_name = lastPrice)]
    pub fn last_price(&self) -> String {
        self.0.last_price.clone()
    }

    #[wasm_bindgen(getter, js_name = highPrice)]
    pub fn high_price(&self) -> String {
        self.0.high_price.clone()
    }

    #[wasm_bindgen(getter, js_name = lowPrice)]
    pub fn low_price(&self) -> String {
        self.0.low_price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn volume(&self) -> String {
        self.0.volume.clone()
    }

    #[wasm_bindgen(getter, js_name = quoteVolume)]
    pub fn quote_volume(&self) -> String {
        self.0.quote_volume.clone()
    }

    #[wasm_bindgen(getter, js_name = openTime)]
    pub fn open_time(&self) -> f64 {
        self.0.open_time as f64
    }

    #[wasm_bindgen(getter, js_name = closeTime)]
    pub fn close_time(&self) -> f64 {
        self.0.close_time as f64
    }

    #[wasm_bindgen(getter, js_name = priceChange)]
    pub fn price_change(&self) -> String {
        self.0.price_change.clone()
    }

    #[wasm_bindgen(getter, js_name = priceChangePercent)]
    pub fn price_change_percent(&self) -> String {
        self.0.price_change_percent.clone()
    }

    #[wasm_bindgen(getter, js_name = weightedAvgPrice)]
    pub fn weighted_avg_price(&self) -> String {
        self.0.weighted_avg_price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn count(&self) -> f64 {
        self.0.count as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TimeResponse
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = TimeResponse)]
pub struct WasmTimeResponse(pub(crate) sdk::TimeResponse);

#[wasm_bindgen(js_class = TimeResponse)]
impl WasmTimeResponse {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter, js_name = serverTime)]
    pub fn server_time(&self) -> f64 {
        self.0.server_time as f64
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Trade
// ══════════════════════════════════════════════════════════════════════════════

#[wasm_bindgen(js_name = Trade)]
pub struct WasmTrade(pub(crate) sdk::Trade);

#[wasm_bindgen(js_class = Trade)]
impl WasmTrade {
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        to_json(&self.0)
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> f64 {
        self.0.id as f64
    }

    #[wasm_bindgen(getter, js_name = isBuyerMaker)]
    pub fn is_buyer_maker(&self) -> bool {
        self.0.is_buyer_maker
    }

    #[wasm_bindgen(getter)]
    pub fn price(&self) -> String {
        self.0.price.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn qty(&self) -> String {
        self.0.qty.clone()
    }

    #[wasm_bindgen(getter, js_name = quoteQty)]
    pub fn quote_qty(&self) -> String {
        self.0.quote_qty.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn time(&self) -> f64 {
        self.0.time as f64
    }
}
