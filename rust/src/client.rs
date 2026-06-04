use std::ops::Deref;
use std::sync::Mutex;

use bon::bon;
use bullet_exchange_interface::message::UserActionDiscriminants;
use bullet_exchange_interface::transaction::{Amount, Gas, PriorityFeeBips};
use bullet_exchange_interface::types::MarketId;
use url::Url;

use crate::generated::Client as GeneratedClient;
use crate::metadata::{ExchangeMetadata, SymbolInfo};
use crate::types::CallMessage;
use crate::{Keypair, SDKError, SDKResult};

/// The main trading API client for REST operations.
///
/// Provides methods for building, signing, and submitting transactions,
/// as well as access to all generated API methods via `Deref`.
///
/// # WebSocket Support
///
/// For real-time market data and WebSocket order submission, use `WsClient`
/// separately. See the `ws` module documentation for details.
///
/// # Example
///
/// ```ignore
/// use bullet_rust_sdk::{Network, Client};
///
/// // Connect to REST API
/// let api = Client::mainnet().await?;
///
/// // Query via REST
/// let info = api.exchange_info().await?;
/// ```
pub struct Client {
    rest_url: String,
    ws_url: String,
    solana_offchain_url: String,
    rollup_url: String,
    generated_client: GeneratedClient,
    pub(crate) http_client: reqwest::Client,
    pub(crate) ws_client: reqwest::Client,
    chain_id: u64,
    chain_hash: Mutex<[u8; 32]>,
    chain_name: String,
    user_actions: Option<Vec<UserActionDiscriminants>>,

    keypair: Option<Keypair>,

    // Exchange metadata (symbol lookups)
    metadata: ExchangeMetadata,

    // Transaction Options
    max_priority_fee_bips: PriorityFeeBips,
    /// The max fee one is willing to pay for this transaction.
    max_fee: Amount,
    /// Optionally limit the number of gas to be used.
    gas_limit: Option<Gas>,
}

/// Known network environments.
///
/// Use with the Client builder to connect to a known network by name,
/// or provide a custom URL.
///
/// # Example
///
/// ```ignore
/// use bullet_rust_sdk::{Client, Network};
///
/// // Known network
/// let client = Client::builder().network(Network::Testnet).build().await?;
///
/// // Custom URL (auto-converts via From<&str>)
/// let client = Client::builder().network("https://custom.example.com").build().await?;
/// ```
#[derive(Debug, Clone)]
pub enum Network {
    Mainnet,
    Testnet,
    Custom(String),
}

impl Network {
    /// Get the REST API URL for this network.
    pub fn url(&self) -> &str {
        match self {
            Network::Mainnet => "https://tradingapi.bullet.xyz",
            Network::Testnet => "https://tradingapi.testnet.bullet.xyz",
            Network::Custom(url) => url,
        }
    }

    /// The rollup host base URL.
    ///
    /// The public networks front a separate rollup host that serves endpoints
    /// the trading API does not proxy — the Solana offchain sequencer and
    /// credential dedup (`/rollup/addresses/.../dedup`). Custom deployments are
    /// assumed to serve the rollup on the same host as the REST API.
    pub fn rollup_url(&self) -> SDKResult<String> {
        match self {
            Network::Mainnet => Ok("https://rollup.mainnet.bullet.xyz".to_string()),
            Network::Testnet => Ok("https://rollup.testnet.bullet.xyz".to_string()),
            Network::Custom(url) => {
                let parsed = Url::parse(url).map_err(|_| SDKError::InvalidNetworkUrl)?;
                let host = parsed.host_str().ok_or(SDKError::InvalidNetworkUrl)?;
                // A custom URL pointing at a canonical trading-API host still
                // needs the matching rollup host (the trading API does not
                // proxy `/rollup/*`). Matching on host — rather than the exact
                // URL string in `From<&str>` — also covers explicit ports and
                // directly-constructed `Network::Custom`.
                match host {
                    "tradingapi.bullet.xyz" => Network::Mainnet.rollup_url(),
                    "tradingapi.testnet.bullet.xyz" => Network::Testnet.rollup_url(),
                    // Other (self-hosted/local) deployments serve the rollup on
                    // the same host. Build from scheme + host + port so any
                    // embedded userinfo (`user:pass@`) is dropped.
                    _ => Ok(match parsed.port() {
                        Some(port) => format!("{}://{host}:{port}", parsed.scheme()),
                        None => format!("{}://{host}", parsed.scheme()),
                    }),
                }
            }
        }
    }
}

impl From<&str> for Network {
    fn from(s: &str) -> Self {
        // Recognize the canonical trading-API URLs as their named networks so
        // the typed `Network` carries the identity downstream (e.g. for
        // `rollup_url`), rather than degrading to an opaque `Custom`.
        match s.to_lowercase().trim_end_matches('/') {
            "mainnet" | "https://tradingapi.bullet.xyz" => Network::Mainnet,
            "testnet" | "https://tradingapi.testnet.bullet.xyz" => Network::Testnet,
            _ => Network::Custom(s.to_string()),
        }
    }
}

impl From<String> for Network {
    fn from(s: String) -> Self {
        Network::from(s.as_str())
    }
}

pub struct ChainData {
    pub chain_hash: [u8; 32],
    pub chain_id: u64,
    pub chain_name: String,
}

pub const MAX_FEE: &Amount = &Amount(10000000000_u128);
pub const MAX_PRIORITY_FEE_BIPS: &PriorityFeeBips = &PriorityFeeBips(0);

#[bon]
impl Client {
    /// Create a new Client connected to a network.
    #[builder]
    pub async fn new(
        #[builder(into)] network: Network,
        /// Custom reqwest client for REST requests.
        ///
        /// **Note:** WebSocket connections use a separate HTTP/1.1 client that does
        /// not inherit settings from this client (e.g. proxy, TLS roots). This is a
        /// reqwest limitation — existing clients can't be reconfigured after construction.
        reqwest_client: Option<reqwest::Client>,
        max_priority_fee_bips: Option<PriorityFeeBips>,
        max_fee: Option<Amount>,
        gas_limit: Option<Gas>,
        keypair: Option<Keypair>,
        /// Override the Solana offchain sequencer endpoint.
        ///
        /// By default this is derived from the selected Bullet network and used
        /// by `Client::send_offchain_transaction`. Overriding it does not change
        /// the rollup host used by `Client::credential_nonce` (that always comes
        /// from the network); set them in tandem if your rollup lives elsewhere.
        #[builder(into)]
        solana_offchain_url: Option<String>,
        /// Restrict schema validation to specific `UserAction` variants.
        ///
        /// By default (`None`), the client validates every exchange `CallMessage`
        /// branch (`User`, `Vault`, `Keeper`, `Public`, and `Admin`) against the remote
        /// schema and returns `SDKError::SchemaOutdated` if any differ. If you only use
        /// a subset of user actions (e.g. `PlaceOrders`), you can pass them here to
        /// intentionally prune validation down to those `UserAction` variants.
        ///
        /// **Warning:** When this is set, non-`User` call messages and unlisted
        /// `UserAction` variants are rejected before signing because their schema branch
        /// was not validated at connect time.
        user_actions: Option<Vec<UserActionDiscriminants>>,
    ) -> SDKResult<Self> {
        let url = network.url();
        let parsed = Url::parse(url).map_err(|_| SDKError::InvalidNetworkUrl)?;

        let (rest_url, ws_url) = match parsed.scheme() {
            "https" => (url.to_string(), format!("wss://{}/ws", parsed.authority())),
            "http" => (url.to_string(), format!("ws://{}/ws", parsed.authority())),
            _ => return Err(SDKError::InvalidNetworkUrl),
        };
        let http_client = reqwest_client.unwrap_or_default();
        let generated_client = GeneratedClient::new_with_client(&rest_url, http_client.clone());
        let rollup_url = network.rollup_url()?;
        let solana_offchain_url = solana_offchain_url
            .unwrap_or_else(|| format!("{rollup_url}/sequencer/solana_offchain_txs"));

        // WebSocket requires HTTP/1.1 (HTTP/2 does not support the Upgrade mechanism).
        // We always build a dedicated HTTP/1.1 client for WS, regardless of whether
        // the caller supplied a custom reqwest client for REST.
        #[cfg(not(target_arch = "wasm32"))]
        let ws_client = reqwest::Client::builder().http1_only().build()?;
        #[cfg(target_arch = "wasm32")]
        let ws_client = reqwest::Client::new();

        // fetch schema
        let chain_data = Self::fetch_schema(&generated_client, &user_actions).await?;

        let max_priority_fee_bips = max_priority_fee_bips.unwrap_or(*MAX_PRIORITY_FEE_BIPS);
        let max_fee = max_fee.unwrap_or(*MAX_FEE);

        let exchange_info = generated_client.exchange_info().await?;
        let metadata = ExchangeMetadata::from_symbols(&exchange_info.into_inner().symbols);

        Ok(Self {
            rest_url,
            ws_url,
            solana_offchain_url,
            rollup_url,
            generated_client,
            http_client,
            ws_client,
            chain_id: chain_data.chain_id,
            chain_hash: Mutex::new(chain_data.chain_hash),
            chain_name: chain_data.chain_name,
            user_actions,
            gas_limit,
            max_priority_fee_bips,
            max_fee,
            keypair,
            metadata,
        })
    }

    async fn fetch_schema(
        generated_client: &GeneratedClient,
        user_actions: &Option<Vec<UserActionDiscriminants>>,
    ) -> SDKResult<ChainData> {
        use bullet_exchange_interface::schema::{Schema, SchemaFile, trim};
        use bullet_exchange_interface::transaction::Transaction;

        let schema_obj = generated_client.schema().await?;

        // validate the remote schema
        let obj = schema_obj.into_inner();
        let sobj = serde_json::to_string(&obj)
            .map_err(|_| SDKError::InvalidSchemaResponse("failed to serialize schema"))?;
        let schema_file = serde_json::from_str::<SchemaFile>(&sobj)
            .map_err(|_| SDKError::InvalidSchemaResponse("failed to parse SchemaFile"))?;
        let our_schema = Schema::of_single_type::<Transaction>()
            .map_err(|_| SDKError::InvalidSchemaResponse("failed to derive local schema"))?;
        let filter = |name: &str, variant: &str| {
            Self::filter_variants(name, variant, user_actions.as_deref())
        };
        let left = trim(&our_schema, &filter);
        let right = trim(&schema_file.schema, &filter);
        if left != right {
            return Err(SDKError::SchemaOutdated);
        }

        // get chain_hash
        let chain_hash_bytes = hex::decode(schema_file.chain_hash.replace("0x", ""))
            .map_err(|e| SDKError::InvalidChainHash(e.to_string()))?;

        let chain_hash = chain_hash_bytes.try_into().map_err(|v: Vec<u8>| {
            SDKError::InvalidChainHash(format!("expected 32 bytes, got {}", v.len()))
        })?;
        let chain_id = schema_file.schema.chain_data().chain_id;
        let chain_name = schema_file.schema.chain_data().chain_name.clone();
        Ok(ChainData { chain_hash, chain_id, chain_name })
    }

    pub async fn update_schema(&self) -> SDKResult<()> {
        let chain_data = Self::fetch_schema(self.client(), self.user_actions()).await?;

        // The expect is fine here as we just read and write the
        // object. We never hold a lock in code that can panic.
        *self.chain_hash.lock().expect("Taking the chain-hash lock can never fail.") =
            chain_data.chain_hash;
        Ok(())
    }

    /// Decides whether a given enum variant should be included in the schema
    /// comparison between our compiled types and the remote API.
    ///
    /// Called by [`trim`] for every `(enum_name, variant_name)` pair in the
    /// schema tree. Returning `true` keeps the variant; `false` prunes it
    /// from both sides before diffing so that changes to pruned variants
    /// don't trigger [`SDKError::SchemaOutdated`].
    ///
    /// The fixed rules pin the transaction envelope the SDK serializes:
    ///   `Transaction::V0 → RuntimeCall::Exchange → CallMessage::*`
    ///
    /// By default, every exchange `CallMessage` group is kept. When `user_actions`
    /// is set, validation is intentionally pruned to:
    ///   `CallMessage::User → selected UserAction::*`
    ///
    /// For `UserAction`, the behaviour depends on `user_actions`:
    /// - `None` — include every variant (full exchange schema check).
    /// - `Some(&[PlaceOrders, CancelOrders])` — only include those two; schema changes to other
    ///   actions (e.g. `Withdraw`) are ignored.
    ///
    /// Unknown enum names default to `true` so any new enums in the schema
    /// are kept, ensuring the diff still catches unexpected additions.
    fn filter_variants(
        name: &str,
        variant: &str,
        user_actions: Option<&[UserActionDiscriminants]>,
    ) -> bool {
        match name {
            "Transaction" => variant == "V0",
            "RuntimeCall" => variant == "Exchange",
            "CallMessage" => user_actions.is_none() || variant == "User",
            "UserAction" => match user_actions {
                Some(actions) => UserActionDiscriminants::try_from(variant)
                    .map(|v| actions.contains(&v))
                    .unwrap_or(false),
                None => true,
            },
            "UniquenessData" => variant == "Generation",
            _ => {
                // include the variant - to be sure we fail afterwards
                true
            }
        }
    }

    pub(crate) fn call_message_was_validated(
        call_message: &CallMessage,
        user_actions: Option<&[UserActionDiscriminants]>,
    ) -> bool {
        let Some(user_actions) = user_actions else {
            return true;
        };

        match call_message {
            CallMessage::User(action) => user_actions.contains(&action.into()),
            _ => false,
        }
    }

    /// Connect to the mainnet environment.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::Client;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let info = api.exchange_info().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn mainnet() -> SDKResult<Self> {
        Self::builder().network(Network::Mainnet).build().await
    }

    /// Get a reference to the underlying generated client.
    ///
    /// Prefer using `Deref` (calling methods directly on `Client`)
    /// instead of this method.
    pub fn client(&self) -> &GeneratedClient {
        &self.generated_client
    }

    /// Get the chain ID for this network.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get the current chain hash.
    pub fn chain_hash(&self) -> [u8; 32] {
        // The expect is fine here as we just read and write the
        // object. We never hold a lock in code that can panic.
        *self.chain_hash.lock().expect("Taking the chain-hash lock can never fail.")
    }

    /// Get the chain name for this network.
    pub fn chain_name(&self) -> String {
        self.chain_name.clone()
    }

    pub fn user_actions(&self) -> &Option<Vec<UserActionDiscriminants>> {
        &self.user_actions
    }

    /// Fetch the current transaction nonce for a credential from the rollup.
    ///
    /// Use with [`UniquenessData::Nonce`](crate::UniquenessData) — e.g. for
    /// multisig transactions, pass
    /// [`MultisigConfig::credential_id`](crate::MultisigConfig::credential_id).
    ///
    /// Queries `GET /rollup/addresses/{credential_id_hex}/dedup` on the
    /// network's rollup host (the trading API does not proxy this endpoint).
    pub async fn credential_nonce(&self, credential_id: &[u8; 32]) -> SDKResult<u64> {
        let url =
            format!("{}/rollup/addresses/{}/dedup", self.rollup_url, hex::encode(credential_id));
        let response = self.http_client.get(url).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(SDKError::RequestError(format!(
                "dedup request failed with status {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        value.get("nonce").and_then(serde_json::Value::as_u64).ok_or_else(|| {
            SDKError::RequestError(format!(
                "dedup response missing numeric `nonce` field: {}",
                String::from_utf8_lossy(&bytes)
            ))
        })
    }

    /// The REST API URL.
    pub fn url(&self) -> &str {
        &self.rest_url
    }
    /// The websocket URL.
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// The Solana offchain sequencer URL.
    pub fn solana_offchain_url(&self) -> &str {
        &self.solana_offchain_url
    }

    /// The rollup host base URL.
    ///
    /// Serves endpoints the trading API does not proxy — the Solana offchain
    /// sequencer and credential dedup (used by [`Client::credential_nonce`]).
    pub fn rollup_url(&self) -> &str {
        &self.rollup_url
    }

    /// Get the default keypair for signing transactions.
    pub fn keypair(&self) -> Option<&Keypair> {
        self.keypair.as_ref()
    }

    /// Get the default max fee for transactions.
    pub fn max_fee(&self) -> Amount {
        self.max_fee
    }

    /// Get the default max priority fee in basis points.
    pub fn max_priority_fee_bips(&self) -> PriorityFeeBips {
        self.max_priority_fee_bips
    }

    /// Get the default gas limit for transactions.
    pub fn gas_limit(&self) -> Option<Gas> {
        self.gas_limit.clone()
    }

    // ── Symbol / Market Lookups ─────────────────────────────────────────

    /// Resolve a symbol string to its [`MarketId`].
    ///
    /// Returns `None` if the symbol is not found in the cached metadata.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let market_id = client.market_id("BTC-USD").expect("unknown symbol");
    /// client.place_orders(market_id, orders, false, None).await?;
    /// ```
    pub fn market_id(&self, symbol: &str) -> Option<MarketId> {
        self.metadata.market_id(symbol)
    }

    /// Get all available symbols and their metadata.
    pub fn symbols(&self) -> &[SymbolInfo] {
        self.metadata.symbols()
    }

    /// Look up symbol info by [`MarketId`].
    pub fn symbol_info(&self, market_id: MarketId) -> Option<&SymbolInfo> {
        self.metadata.symbol_info_by_id(market_id)
    }

    /// Look up symbol info by name.
    pub fn symbol_info_by_name(&self, symbol: &str) -> Option<&SymbolInfo> {
        self.metadata.symbol_info_by_name(symbol)
    }

    /// Re-fetch exchange metadata from the server.
    ///
    /// Call this in long-running bots to pick up newly listed markets.
    pub async fn refresh_metadata(&mut self) -> SDKResult<()> {
        let info = self.generated_client.exchange_info().await?;
        self.metadata = ExchangeMetadata::from_symbols(&info.into_inner().symbols);
        Ok(())
    }
}

/// Implement Deref to allow calling generated client methods directly.
///
/// This enables ergonomic access to all API methods:
///
/// ```ignore
/// let api = Client::new(url, None).await?;
/// let info = api.exchange_info().await?;
/// ```
impl Deref for Client {
    type Target = GeneratedClient;

    fn deref(&self) -> &Self::Target {
        self.client()
    }
}

#[cfg(test)]
mod tests {
    use bullet_exchange_interface::message::PublicAction;

    use super::*;
    use crate::types::UserAction;

    #[test]
    fn network_recognizes_canonical_trading_api_urls() {
        assert!(matches!(Network::from("https://tradingapi.bullet.xyz"), Network::Mainnet));
        assert!(matches!(Network::from("https://tradingapi.bullet.xyz/"), Network::Mainnet));
        assert!(matches!(Network::from("https://tradingapi.testnet.bullet.xyz"), Network::Testnet));
        assert!(matches!(Network::from("https://custom.example.com"), Network::Custom(_)));
    }

    #[test]
    fn rollup_url_maps_public_networks_to_rollup_host() {
        // The trading API does not proxy /rollup/* — the dedup endpoint must
        // target the rollup host, not tradingapi.
        assert_eq!(Network::Mainnet.rollup_url().unwrap(), "https://rollup.mainnet.bullet.xyz");
        assert_eq!(Network::Testnet.rollup_url().unwrap(), "https://rollup.testnet.bullet.xyz");
    }

    #[test]
    fn rollup_url_derives_custom_hosts_from_their_authority() {
        assert_eq!(
            Network::from("http://localhost:8080").rollup_url().unwrap(),
            "http://localhost:8080"
        );
        // A custom host with a path keeps only scheme + authority.
        assert_eq!(
            Network::from("https://staging.example.com/api").rollup_url().unwrap(),
            "https://staging.example.com"
        );
        // Any embedded userinfo is dropped.
        assert_eq!(
            Network::from("https://user:pass@staging.example.com").rollup_url().unwrap(),
            "https://staging.example.com"
        );
    }

    #[test]
    fn rollup_url_maps_custom_canonical_hosts_to_rollup() {
        // A `Custom` built directly with a canonical trading-API host (bypassing
        // `From<&str>`), or with an explicit port, still routes to the rollup.
        assert_eq!(
            Network::Custom("https://tradingapi.bullet.xyz".to_string()).rollup_url().unwrap(),
            "https://rollup.mainnet.bullet.xyz"
        );
        assert_eq!(
            Network::Custom("https://tradingapi.testnet.bullet.xyz:443".to_string())
                .rollup_url()
                .unwrap(),
            "https://rollup.testnet.bullet.xyz"
        );
    }

    #[test]
    fn default_schema_filter_keeps_all_call_message_groups() {
        for variant in ["User", "Vault", "Keeper", "Public", "Admin"] {
            assert!(Client::filter_variants("CallMessage", variant, None));
        }
    }

    #[test]
    fn selective_schema_filter_keeps_only_user_call_messages() {
        let selected = [UserActionDiscriminants::PlaceOrders];

        assert!(Client::filter_variants("CallMessage", "User", Some(&selected)));
        for variant in ["Vault", "Keeper", "Public", "Admin"] {
            assert!(!Client::filter_variants("CallMessage", variant, Some(&selected)));
        }
    }

    #[test]
    fn selective_schema_filter_keeps_only_selected_user_actions() {
        let selected = [UserActionDiscriminants::PlaceOrders];

        assert!(Client::filter_variants("UserAction", "PlaceOrders", Some(&selected)));
        assert!(!Client::filter_variants("UserAction", "CancelOrders", Some(&selected)));
    }

    #[test]
    fn selective_mode_rejects_unvalidated_call_messages() {
        let selected = [UserActionDiscriminants::PlaceOrders];
        let public_call = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });
        let unselected_user_call =
            CallMessage::User(UserAction::CancelAllOrders { sub_account_index: None });

        assert!(!Client::call_message_was_validated(&public_call, Some(&selected)));
        assert!(!Client::call_message_was_validated(&unselected_user_call, Some(&selected),));
    }
}
