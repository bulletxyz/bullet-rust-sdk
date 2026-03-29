use bon::bon;
use bullet_exchange_interface::message::UserActionDiscriminants;
use bullet_exchange_interface::transaction::{Amount, Gas, PriorityFeeBips};
use std::ops::Deref;

use crate::generated::Client as GeneratedClient;
use crate::{Keypair, SDKError, SDKResult};
use url::Url;

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
    chain_id: u64,
    chain_hash: [u8; 32],

    keypair: Option<Keypair>,

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

pub const MAX_FEE: &'static Amount = &Amount(10000000000_u128);
pub const MAX_PRIORITY_FEE_BIPS: &'static PriorityFeeBips = &PriorityFeeBips(0);

#[bon]
impl Client {
    /// Create a new Client connected to a network.
    #[builder]
    pub async fn new(
        #[builder(into)]
        network: Network,
        reqwest_client: Option<reqwest::Client>,
        max_priority_fee_bips: Option<PriorityFeeBips>,
        max_fee: Option<Amount>,
        gas_limit: Option<Gas>,
        keypair: Option<Keypair>,
    ) -> SDKResult<Self> {
        use bullet_exchange_interface::schema::{trim, Schema, SchemaFile};
        use bullet_exchange_interface::transaction::Transaction;

        let url = network.url();
        let parsed = Url::parse(url).map_err(|_| SDKError::InvalidNetworkUrl)?;

        let (rest_url, ws_url) = match parsed.scheme() {
            "https" => (url.to_string(), format!("wss://{}/ws", parsed.authority())),
            "http" => (url.to_string(), format!("ws://{}/ws", parsed.authority())),
            _ => return Err(SDKError::InvalidNetworkUrl),
        };
        let generated_client = match reqwest_client {
            Some(client) => GeneratedClient::new_with_client(&rest_url, client),
            None => GeneratedClient::new(&rest_url),
        };

        // fetch schema
        let schema_obj = generated_client.schema().await?;

        // validate the remote schema
        let obj = schema_obj.into_inner();
        let sobj = serde_json::to_string(&obj).unwrap();
        let schema_file = serde_json::from_str::<SchemaFile>(&sobj).unwrap();
        let our_schema = Schema::of_single_type::<Transaction>().unwrap();
        let left = trim(&our_schema, &Self::filter_variants);
        let right = trim(&schema_file.schema, &Self::filter_variants);
        if left != right {
            return Err(SDKError::SchemaOutdated);
        }

        // get chain_hash
        let chain_hash_bytes = hex::decode(schema_file.chain_hash.replace("0x", ""))
            .map_err(|e| SDKError::InvalidChainHash(e.to_string()))?;

        let chain_hash = chain_hash_bytes.try_into().map_err(|v: Vec<u8>| {
            SDKError::InvalidChainHash(format!("expected 32 bytes, got {}", v.len()))
        })?;

        // get chain-id - XXX unfortunatelly this field is private upstream
        let chain_id = obj
            .get("schema")
            .and_then(|x| x.get("chain_data"))
            .and_then(|x| x.get("chain_id"))
            .and_then(|x| x.as_u64())
            .ok_or(SDKError::InvalidSchemaResponse("chain_id"))?;

        let max_priority_fee_bips = max_priority_fee_bips.unwrap_or_else(|| *MAX_PRIORITY_FEE_BIPS);
        let max_fee = max_fee.unwrap_or_else(|| *MAX_FEE);

        Ok(Self {
            rest_url,
            ws_url,
            generated_client,
            chain_id,
            chain_hash,
            gas_limit,
            max_priority_fee_bips,
            max_fee,
            keypair,
        })
    }

    /// This is a white-list of Transaction variants that must not
    /// change to not break our binary.
    fn filter_variants(name: &str, variant: &str) -> bool {
        match name {
            "Transaction" => variant == "V0",
            "RuntimeCall" => variant == "Exchange",
            "CallMessage" => variant == "User",
            "UserAction" => UserActionDiscriminants::try_from(variant).is_ok(),
            "UniquenessData" => variant == "Generation",
            _ => {
                // include the variant - to be sure we fail afterwards
                true
            }
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
    pub fn chain_hash(&self) -> &[u8; 32] {
        &self.chain_hash
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

    /// Clone the underlying `reqwest::Client` for use in reconnection loops.
    pub(crate) fn http_client(&self) -> reqwest::Client {
        self.generated_client.client.clone()
    }

    /// Connect to the WebSocket API with automatic reconnection.
    ///
    /// Returns a [`ManagedWebsocket`] that reconnects on disconnect and replays
    /// all active subscriptions. Eagerly connects — errors surface immediately.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::{Client, Topic};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let mut ws = api.connect_ws_managed().await?;
    ///
    /// ws.subscribe([Topic::agg_trade("BTC-USD")], None).await?;
    ///
    /// loop {
    ///     let msg = ws.recv().await?; // reconnects automatically
    ///     println!("{:?}", msg);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_ws_managed(
        &self,
    ) -> SDKResult<crate::ws::ManagedWebsocket> {
        crate::ws::ManagedWebsocket::new(self).await.map_err(Into::into)
    }

    /// Look up the numeric `MarketId` for a symbol string (e.g. `"BTC-USD"`).
    ///
    /// Calls the exchange info endpoint on each invocation; results are not cached.
    /// For high-frequency use, resolve the `MarketId` once and reuse it.
    ///
    /// # Errors
    ///
    /// Returns [`SDKError::SymbolNotFound`] if the symbol is not listed on the exchange.
    pub async fn market_id_for(
        &self,
        symbol: &str,
    ) -> crate::SDKResult<bullet_exchange_interface::types::MarketId> {
        let info = self.exchange_info().await?;
        info.into_inner()
            .symbols
            .iter()
            .find(|s| s.symbol == symbol)
            .map(|s| bullet_exchange_interface::types::MarketId(s.market_id))
            .ok_or_else(|| crate::SDKError::SymbolNotFound(symbol.to_string()))
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
