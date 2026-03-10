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

    // ── REST endpoints ────────────────────────────────────────────────────────

    /// Returns JSON string of `PingResponse`.
    pub async fn ping(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.ping().await?.into_inner())?)
    }

    /// Returns JSON string of `TimeResponse`.
    pub async fn time(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.time().await?.into_inner())?)
    }

    /// Returns JSON string of `ExchangeInfo`.
    #[wasm_bindgen(js_name = exchangeInfo)]
    pub async fn exchange_info(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.exchange_info().await?.into_inner())?)
    }

    /// Returns JSON string of `OrderBook`. `limit` is optional.
    #[wasm_bindgen(js_name = orderBook)]
    pub async fn order_book(&self, symbol: &str, limit: Option<i32>) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.order_book(limit, symbol).await?.into_inner())?)
    }

    /// Returns JSON string of `Vec<Trade>`. `limit` is optional.
    #[wasm_bindgen(js_name = recentTrades)]
    pub async fn recent_trades(&self, symbol: &str, limit: Option<i32>) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.recent_trades(limit, symbol).await?.into_inner())?)
    }

    /// Returns JSON string of `Ticker24hr`. `symbol` is optional.
    #[wasm_bindgen(js_name = ticker24hr)]
    pub async fn ticker_24hr(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.ticker_24hr(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `Vec<PriceTicker>`. `symbol` is optional.
    #[wasm_bindgen(js_name = tickerPrice)]
    pub async fn ticker_price(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.ticker_price(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `FundingRate`. `symbol` is optional.
    #[wasm_bindgen(js_name = fundingRate)]
    pub async fn funding_rate(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.funding_rate(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `Vec<InsuranceBalance>`.
    #[wasm_bindgen(js_name = insuranceBalance)]
    pub async fn insurance_balance(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.insurance_balance().await?.into_inner())?)
    }

    /// Returns JSON string of `Vec<BorrowLendPoolResponse>`. `symbol` is optional.
    #[wasm_bindgen(js_name = borrowLendPools)]
    pub async fn borrow_lend_pools(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.borrow_lend_pools(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `Vec<LeverageBracket>`. `symbol` is optional.
    #[wasm_bindgen(js_name = leverageBracket)]
    pub async fn leverage_bracket(&self, symbol: Option<String>) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.leverage_bracket(symbol.as_deref()).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `Account`.
    #[wasm_bindgen(js_name = accountInfo)]
    pub async fn account_info(&self, address: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.account_info(address).await?.into_inner())?)
    }

    /// Returns JSON string of `Vec<Balance>`.
    #[wasm_bindgen(js_name = accountBalance)]
    pub async fn account_balance(&self, address: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.account_balance(address).await?.into_inner())?)
    }

    /// Returns JSON string of `BinanceOrder`.
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

    /// Returns JSON string of `Vec<BinanceOrder>`.
    #[wasm_bindgen(js_name = queryOpenOrders)]
    pub async fn query_open_orders(&self, address: &str, symbol: &str) -> WasmResult<String> {
        Ok(serde_json::to_string(
            &self.inner.query_open_orders(address, symbol).await?.into_inner(),
        )?)
    }

    /// Returns JSON string of `RollupConstants`.
    pub async fn constants(&self) -> WasmResult<String> {
        Ok(serde_json::to_string(&self.inner.constants().await?.into_inner())?)
    }
}
