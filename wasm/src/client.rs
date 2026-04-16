use bullet_exchange_interface::message::UserActionDiscriminants;
use bullet_exchange_interface::transaction::{Amount, Gas, PriorityFeeBips};
use bullet_rust_sdk::{Client, Network};
use wasm_bindgen::prelude::*;

use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;

/// Known network constants for connecting to Bullet environments.
///
/// Use these with `Client.builder().network(Network.Testnet)`,
/// or pass any custom URL string directly.
///
/// # Example
///
/// ```js
/// // Known network
/// const client = await Client.builder().network(Network.Testnet).build();
///
/// // Custom URL
/// const client = await Client.builder().network("https://custom.example.com").build();
/// ```
#[wasm_bindgen(js_name = Network)]
pub struct WasmNetwork;

#[wasm_bindgen(js_class = Network)]
impl WasmNetwork {
    #[wasm_bindgen(getter, js_name = Mainnet)]
    pub fn mainnet() -> String {
        "mainnet".to_string()
    }

    #[wasm_bindgen(getter, js_name = Testnet)]
    pub fn testnet() -> String {
        "testnet".to_string()
    }
}

/// Full Bullet trading API client (REST + WebSocket).
///
/// All REST responses are returned as JSON strings.
/// Errors are thrown as JavaScript `Error` objects with a `.message` property.
///
/// # Example
///
/// ```js
/// // Simple connection
/// const client = await Client.mainnet();
///
/// // With defaults for transactions
/// const client = await Client.builder()
///     .network(Network.Testnet)
///     .keypair(myKeypair)
///     .maxFee(10_000_000n)
///     .build();
/// ```
#[wasm_bindgen(js_name = Client)]
pub struct WasmTradingApi {
    pub(crate) inner: Client,
}

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Create a new client builder for configuring connection options.
    ///
    /// # Example
    ///
    /// ```js
    /// const client = await Client.builder()
    ///     .network(Network.Testnet)
    ///     .keypair(myKeypair)
    ///     .maxFee(10_000_000n)
    ///     .build();
    /// ```
    pub fn builder() -> WasmClientBuilder {
        WasmClientBuilder::new()
    }

    /// Connect to the mainnet REST endpoint and validate the remote schema.
    pub async fn mainnet() -> WasmResult<WasmTradingApi> {
        Ok(WasmTradingApi {
            inner: Client::mainnet().await?,
        })
    }

    /// Connect to a network by name or custom URL.
    ///
    /// For more options, use `Client.builder()` instead.
    pub async fn connect(network: &str) -> WasmResult<WasmTradingApi> {
        Ok(WasmTradingApi {
            inner: Client::builder()
                .network(Network::from(network))
                .build()
                .await?,
        })
    }

    // ── Metadata ──────────────────────────────────────────────────────────────

    /// Chain ID for the connected network.
    #[wasm_bindgen(js_name = chainId)]
    pub fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    /// Chain hash as bytes.
    #[wasm_bindgen(js_name = chainHash)]
    pub fn chain_hash(&self) -> Vec<u8> {
        self.inner.chain_hash().to_vec()
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

    /// Get the default max fee for transactions.
    #[wasm_bindgen(js_name = maxFee)]
    pub fn max_fee(&self) -> u64 {
        self.inner.max_fee().0 as u64
    }

    /// Get the default max priority fee in basis points.
    #[wasm_bindgen(js_name = maxPriorityFeeBips)]
    pub fn max_priority_fee_bips(&self) -> u64 {
        self.inner.max_priority_fee_bips().0
    }

    /// Get the default gas limit for transactions (if set).
    /// Returns [ref_time, proof_size] as a two-element array.
    #[wasm_bindgen(js_name = gasLimit)]
    pub fn gas_limit(&self) -> Option<Vec<u64>> {
        self.inner.gas_limit().map(|g| g.0.to_vec())
    }

    /// Check if a default keypair is configured.
    #[wasm_bindgen(js_name = hasKeypair)]
    pub fn has_keypair(&self) -> bool {
        self.inner.keypair().is_some()
    }

    // ── Symbol / Market Lookups ──────────────────────────────────────────

    /// Resolve a symbol string to its numeric MarketId.
    /// @param {string} symbol - The trading pair (e.g. "BTC-USD").
    /// @returns {number | undefined}
    #[wasm_bindgen(js_name = marketId)]
    pub fn market_id(&self, symbol: &str) -> Option<u16> {
        self.inner.market_id(symbol).map(|m| m.0)
    }

    /// Get all available symbols as a JSON array.
    /// @returns {string} JSON array of `{ symbol, marketId, status, baseAsset, quoteAsset, pricePrecision, quantityPrecision }`
    pub fn symbols(&self) -> String {
        let symbols: Vec<serde_json::Value> = self
            .inner
            .symbols()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "symbol": s.symbol,
                    "marketId": s.market_id.0,
                    "status": s.status,
                    "baseAsset": s.base_asset,
                    "quoteAsset": s.quote_asset,
                    "pricePrecision": s.price_precision,
                    "quantityPrecision": s.quantity_precision,
                })
            })
            .collect();
        serde_json::to_string(&symbols).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get the base58 address derived from the client's keypair.
    /// @returns {string}
    pub fn address(&self) -> WasmResult<String> {
        Ok(self.inner.address()?)
    }

    /// Re-fetch exchange metadata from the server.
    /// Call this in long-running bots to pick up newly listed markets.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = refreshMetadata)]
    pub async fn refresh_metadata(&mut self) -> WasmResult<()> {
        Ok(self.inner.refresh_metadata().await?)
    }

    // ── Trading Convenience Methods ──────────────────────────────────────

    /// Query open orders for the client's own account.
    /// @param {string} symbol - Trading pair symbol.
    /// @returns {Promise<string>} JSON array of orders.
    #[wasm_bindgen(js_name = myOpenOrders)]
    pub async fn my_open_orders(&self, symbol: &str) -> WasmResult<String> {
        let orders = self.inner.my_open_orders(symbol).await?;
        Ok(serde_json::to_string(&orders)?)
    }

    /// Query account info (positions, margins) for the client's own account.
    /// @returns {Promise<string>} JSON object.
    #[wasm_bindgen(js_name = myAccount)]
    pub async fn my_account(&self) -> WasmResult<String> {
        let account = self.inner.my_account().await?;
        Ok(serde_json::to_string(&account)?)
    }

    /// Query balances for the client's own account.
    /// @returns {Promise<string>} JSON array of balances.
    #[wasm_bindgen(js_name = myBalances)]
    pub async fn my_balances(&self) -> WasmResult<String> {
        let balances = self.inner.my_balances().await?;
        Ok(serde_json::to_string(&balances)?)
    }

    /// Cancel all orders on a specific market.
    /// @param {number} marketId - Numeric market ID.
    /// @param {number} [subAccountIndex] - Optional sub-account index.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = cancelMarketOrders)]
    pub async fn cancel_market_orders(
        &self,
        market_id: u16,
        sub_account_index: Option<u8>,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self
            .inner
            .cancel_market_orders(
                bullet_exchange_interface::types::MarketId(market_id),
                sub_account_index,
            )
            .await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Cancel all orders across all markets.
    /// @param {number} [subAccountIndex] - Optional sub-account index.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = cancelAllOrders)]
    pub async fn cancel_all_orders(
        &self,
        sub_account_index: Option<u8>,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.cancel_all_orders(sub_account_index).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }
}

/// Builder for creating a Client with custom configuration.
///
/// # Example
///
/// ```js
/// const client = await Client.builder()
///     .network(Network.Testnet)
///     .keypair(myKeypair)
///     .maxFee(10_000_000n)
///     .maxPriorityFeeBips(100n)
///     .build();
/// ```
#[wasm_bindgen(js_name = ClientBuilder)]
pub struct WasmClientBuilder {
    network: Option<String>,
    keypair: Option<WasmKeypair>,
    max_fee: Option<u64>,
    max_priority_fee_bips: Option<u64>,
    gas_limit: Option<[u64; 2]>,
    user_actions: Option<Vec<UserActionDiscriminants>>,
}

impl WasmClientBuilder {
    fn new() -> Self {
        WasmClientBuilder {
            network: None,
            keypair: None,
            max_fee: None,
            max_priority_fee_bips: None,
            gas_limit: None,
            user_actions: None,
        }
    }
}

#[wasm_bindgen(js_class = ClientBuilder)]
impl WasmClientBuilder {
    /// Set the network to connect to (required).
    ///
    /// Accepts a known network name (`Network.Mainnet`, `Network.Testnet`)
    /// or a custom URL string.
    pub fn network(mut self, network: &str) -> WasmClientBuilder {
        self.network = Some(network.to_string());
        self
    }

    /// Set the default keypair for signing transactions.
    pub fn keypair(mut self, keypair: WasmKeypair) -> WasmClientBuilder {
        self.keypair = Some(keypair);
        self
    }

    /// Set the default maximum fee (in base units) for transactions.
    #[wasm_bindgen(js_name = maxFee)]
    pub fn max_fee(mut self, fee: u64) -> WasmClientBuilder {
        self.max_fee = Some(fee);
        self
    }

    /// Set the default priority fee in basis points.
    #[wasm_bindgen(js_name = maxPriorityFeeBips)]
    pub fn max_priority_fee_bips(mut self, bips: u64) -> WasmClientBuilder {
        self.max_priority_fee_bips = Some(bips);
        self
    }

    /// Set the default gas limit for transactions.
    /// Takes [ref_time, proof_size] as parameters.
    #[wasm_bindgen(js_name = gasLimit)]
    pub fn gas_limit(mut self, ref_time: u64, proof_size: u64) -> WasmClientBuilder {
        self.gas_limit = Some([ref_time, proof_size]);
        self
    }

    /// Restrict schema validation to specific `UserAction` variants.
    ///
    /// Pass an array of action name strings (e.g. `["PlaceOrders", "CancelOrders"]`).
    /// When set, only these actions are checked against the remote schema — changes
    /// to other actions won't prevent connection.
    ///
    /// **Warning:** If you use an action not listed here, the client will silently
    /// skip its schema check — a breaking change to that action's schema won't be
    /// caught at connect time and may cause runtime serialization failures.
    #[wasm_bindgen(js_name = userActions)]
    pub fn user_actions(mut self, actions: Vec<String>) -> Result<WasmClientBuilder, JsError> {
        let parsed: Result<Vec<UserActionDiscriminants>, _> = actions
            .iter()
            .map(|s| {
                UserActionDiscriminants::try_from(s.as_str())
                    .map_err(|_| JsError::new(&format!("unknown UserAction variant: {s}")))
            })
            .collect();
        self.user_actions = Some(parsed?);
        Ok(self)
    }

    /// Build the client and connect to the API.
    pub async fn build(self) -> WasmResult<WasmTradingApi> {
        let network: Network = self.network.ok_or("network is required")?.as_str().into();

        let keypair = self.keypair.map(|k| k.inner);
        let max_fee = self.max_fee.map(|f| Amount(f as u128));
        let max_priority_fee_bips = self.max_priority_fee_bips.map(PriorityFeeBips);
        let gas_limit = self.gas_limit.map(Gas);

        let inner = Client::builder()
            .network(network)
            .maybe_keypair(keypair)
            .maybe_max_fee(max_fee)
            .maybe_max_priority_fee_bips(max_priority_fee_bips)
            .maybe_gas_limit(gas_limit)
            .maybe_user_actions(self.user_actions)
            .build()
            .await?;

        Ok(WasmTradingApi { inner })
    }
}
