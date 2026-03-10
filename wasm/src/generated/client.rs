//! Wasm-bindgen wrappers for all progenitor-generated REST endpoints.
//!
//! Each method delegates directly to the corresponding `bullet_rust_sdk::codegen::Client`
//! method (via `TradingApi`'s `Deref` impl) and serialises the response to a JSON string.
//!
//! This file mirrors the structure of `src/client.rs` in the base SDK crate and should
//! grow in lockstep with the generated `Client` — the coverage test in
//! `src/coverage_test.rs` enforces that every progenitor operation is accounted for.

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = TradingApi)]
impl WasmTradingApi {
    // ── Connectivity ──────────────────────────────────────────────────────────

    /// Test connectivity. Returns JSON string of `PingResponse`.
    pub async fn ping(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.ping().await?.into_inner())?)
    }

    /// Server time. Returns JSON string of `TimeResponse`.
    pub async fn time(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.time().await?.into_inner())?)
    }

    /// API health check. Returns the raw response body as a string.
    pub async fn health(&self) -> WasmResult<String> {
        use futures_util::TryStreamExt as _;
        let bytes: Vec<u8> = self.inner.health().await?.into_inner().into_inner()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Prometheus metrics. Returns the raw response body as a string.
    pub async fn metrics(&self) -> WasmResult<String> {
        use futures_util::TryStreamExt as _;
        let bytes: Vec<u8> = self.inner.metrics().await?.into_inner().into_inner()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    // ── Market data ───────────────────────────────────────────────────────────

    /// Exchange trading rules and symbol info. Returns JSON string of `ExchangeInfo`.
    #[wasm_bindgen(js_name = exchangeInfo)]
    pub async fn exchange_info(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.exchange_info().await?.into_inner())?)
    }

    /// Order book snapshot. Returns JSON string of `OrderBook`.
    #[wasm_bindgen(js_name = orderBook)]
    pub async fn order_book(&self, symbol: &str, limit: Option<i32>) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.order_book(limit, symbol).await?.into_inner())?)
    }

    /// Recent trades. Returns JSON string of `Vec<Trade>`.
    #[wasm_bindgen(js_name = recentTrades)]
    pub async fn recent_trades(&self, symbol: &str, limit: Option<i32>) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.recent_trades(limit, symbol).await?.into_inner())?)
    }

    /// 24-hour ticker statistics. Returns JSON string of `Ticker24hr`.
    #[wasm_bindgen(js_name = ticker24hr)]
    pub async fn ticker_24hr(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.ticker_24hr(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Latest price for one or all symbols. Returns JSON string of `Vec<PriceTicker>`.
    #[wasm_bindgen(js_name = tickerPrice)]
    pub async fn ticker_price(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.ticker_price(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Funding rate. Returns JSON string of `FundingRate`.
    #[wasm_bindgen(js_name = fundingRate)]
    pub async fn funding_rate(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.funding_rate(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    // ── Account ───────────────────────────────────────────────────────────────

    /// Account information. Returns JSON string of `Account`.
    #[wasm_bindgen(js_name = accountInfo)]
    pub async fn account_info(&self, address: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.account_info(address).await?.into_inner())?)
    }

    /// Account balances. Returns JSON string of `Vec<Balance>`.
    #[wasm_bindgen(js_name = accountBalance)]
    pub async fn account_balance(&self, address: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.account_balance(address).await?.into_inner())?)
    }

    /// Account configuration. Returns the raw response body as a string.
    #[wasm_bindgen(js_name = accountConfig)]
    pub async fn account_config(&self, address: &str) -> WasmResult<String> {
        use futures_util::TryStreamExt as _;
        let bytes: Vec<u8> = self.inner.account_config(address).await?.into_inner().into_inner()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Commission rate for an address and symbol. Returns nothing on success.
    #[wasm_bindgen(js_name = commissionRate)]
    pub async fn commission_rate(&self, address: &str, symbol: &str) -> WasmResult<()> {
        self.inner.commission_rate(address, symbol).await?;
        Ok(())
    }

    /// Symbol configuration for an address. Returns nothing on success.
    #[wasm_bindgen(js_name = symbolConfig)]
    pub async fn symbol_config(&self, address: &str, symbol: Option<String>) -> WasmResult<()> {
        self.inner.symbol_config(address, symbol.as_deref()).await?;
        Ok(())
    }

    /// Rate limit order info. Returns the raw response body as a string.
    #[wasm_bindgen(js_name = rateLimitOrder)]
    pub async fn rate_limit_order(&self) -> WasmResult<String> {
        use futures_util::TryStreamExt as _;
        let bytes: Vec<u8> = self.inner.rate_limit_order().await?.into_inner().into_inner()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    // ── Orders ────────────────────────────────────────────────────────────────

    /// Query a single open order. Returns JSON string of `BinanceOrder`.
    #[wasm_bindgen(js_name = queryOpenOrder)]
    pub async fn query_open_order(
        &self,
        address: &str,
        symbol: &str,
        order_id: Option<i64>,
        client_order_id: Option<i64>,
    ) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self
                .inner
                .query_open_order(address, client_order_id, order_id, symbol)
                .await?
                .into_inner(),
        )?)
    }

    /// Query all open orders. Returns JSON string of `Vec<BinanceOrder>`.
    #[wasm_bindgen(js_name = queryOpenOrders)]
    pub async fn query_open_orders(&self, address: &str, symbol: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.query_open_orders(address, symbol).await?.into_inner(),
        )?)
    }

    /// Notional and leverage brackets. Returns JSON string of `Vec<LeverageBracket>`.
    #[wasm_bindgen(js_name = leverageBracket)]
    pub async fn leverage_bracket(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.leverage_bracket(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    // ── Borrow / insurance ────────────────────────────────────────────────────

    /// Insurance fund balance. Returns JSON string of `Vec<InsuranceBalance>`.
    #[wasm_bindgen(js_name = insuranceBalance)]
    pub async fn insurance_balance(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.insurance_balance().await?.into_inner())?)
    }

    /// Borrow/lend pool info. Returns JSON string of `Vec<BorrowLendPoolResponse>`.
    #[wasm_bindgen(js_name = borrowLendPools)]
    pub async fn borrow_lend_pools(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.borrow_lend_pools(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    // ── Rollup ────────────────────────────────────────────────────────────────

    /// Rollup chain constants. Returns JSON string of `RollupConstants`.
    pub async fn constants(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.constants().await?.into_inner())?)
    }

    /// Rollup schema. Returns the raw JSON schema as a string.
    pub async fn schema(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.schema().await?.into_inner())?)
    }
}
