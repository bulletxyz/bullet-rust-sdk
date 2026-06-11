use std::ops::Deref;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use bon::bon;
use bullet_exchange_interface::message::UserActionDiscriminants;
use bullet_exchange_interface::transaction::{Amount, Gas, PriorityFeeBips};
use bullet_exchange_interface::types::MarketId;
use url::Url;
use web_time::{SystemTime, UNIX_EPOCH};

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
    generated_client: GeneratedClient,
    pub(crate) ws_client: reqwest::Client,
    chain_id: u64,
    chain_hash: Mutex<[u8; 32]>,
    chain_name: String,
    user_actions: Option<Vec<UserActionDiscriminants>>,

    /// Monotonic counter for the default `Window` uniqueness. Each call raises
    /// it to at least the current millisecond unix timestamp and then
    /// increments, so values track the wall clock (independent clients with
    /// synchronised clocks converge and don't collide unless they submit in the
    /// very same millisecond) while staying unique within this client under
    /// sub-millisecond bursts.
    ///
    /// The counter is in-memory: a client restarted *mid-burst* re-seeds from
    /// the wall clock, so a sub-millisecond burst larger than the restart
    /// latency could briefly reuse values (rejected as replays within the
    /// rollup's window). Throughput-sensitive or multi-instance setups that
    /// can't tolerate this should set an explicit `uniqueness` on the builder.
    window_nonce: AtomicU64,

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
}

impl From<&str> for Network {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mainnet" => Network::Mainnet,
            "testnet" => Network::Testnet,
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
            generated_client,
            ws_client,
            chain_id: chain_data.chain_id,
            chain_hash: Mutex::new(chain_data.chain_hash),
            chain_name: chain_data.chain_name,
            user_actions,
            window_nonce: AtomicU64::new(0),
            gas_limit,
            max_priority_fee_bips,
            max_fee,
            keypair,
            metadata,
        })
    }

    /// Return the next value for the default `Window` uniqueness and advance the
    /// counter.
    ///
    /// Raises the counter to at least the current millisecond unix timestamp
    /// (so independent clients with synchronised clocks pick converging values)
    /// and then increments, keeping each value unique and monotonic within this
    /// client even under sub-millisecond bursts.
    pub(crate) fn next_window_nonce(&self) -> u64 {
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        self.window_nonce.fetch_max(now, Ordering::Relaxed);
        self.window_nonce.fetch_add(1, Ordering::Relaxed)
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
            // Validate only the `Generation` arm: it's the uniqueness variant
            // present in every deployed network's schema today. `Window` (the
            // SDK default) and `Nonce` exist in the local interface but aren't
            // on mainnet's schema yet, so checking them here would falsely
            // reject connecting to a mainnet that trails the local interface.
            // Their layout is a plain `u64` pinned by the shared
            // `bullet-exchange-interface` version; broaden this to `Window`
            // once mainnet exposes it.
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

    /// The REST API URL.
    pub fn url(&self) -> &str {
        &self.rest_url
    }
    /// The websocket URL.
    pub fn ws_url(&self) -> &str {
        &self.ws_url
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
    fn network_from_resolves_named_networks_and_custom_urls() {
        assert!(matches!(Network::from("mainnet"), Network::Mainnet));
        assert!(matches!(Network::from("testnet"), Network::Testnet));
        assert!(matches!(Network::from("https://custom.example.com"), Network::Custom(_)));
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
