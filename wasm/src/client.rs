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

    #[wasm_bindgen(getter, js_name = Staging)]
    pub fn staging() -> String {
        "staging".to_string()
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
}

impl WasmClientBuilder {
    fn new() -> Self {
        WasmClientBuilder {
            network: None,
            keypair: None,
            max_fee: None,
            max_priority_fee_bips: None,
            gas_limit: None,
        }
    }
}

#[wasm_bindgen(js_class = ClientBuilder)]
impl WasmClientBuilder {
    /// Set the network to connect to (required).
    ///
    /// Accepts a known network name (`Network.Mainnet`, `Network.Testnet`, `Network.Staging`)
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

    /// Build the client and connect to the API.
    pub async fn build(self) -> WasmResult<WasmTradingApi> {
        let network: Network = self
            .network
            .ok_or("network is required")?
            .as_str()
            .into();

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
            .build()
            .await?;

        Ok(WasmTradingApi { inner })
    }
}
