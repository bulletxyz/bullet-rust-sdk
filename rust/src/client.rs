use bon::bon;
use bullet_exchange_interface::message::UserActionDiscriminants;
use std::fmt::Debug;
use std::ops::Deref;

use crate::generated::Client as GeneratedClient;
use crate::{SDKError, SDKResult};
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
}

pub const MAINNET_URL: &str = "https://tradingapi.bullet.xyz";

#[bon]
impl Client {
    /// Create a new Client from a URL.
    #[builder]
    pub async fn new(url: &str, reqwest_client: Option<reqwest::Client>) -> SDKResult<Self> {
        use bullet_exchange_interface::schema::{trim, Schema, SchemaFile};
        use bullet_exchange_interface::transaction::Transaction;

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
            panic!("Schema outdated - recompile the binary to update bullet-exchange-interface.")
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

        Ok(Self {
            rest_url,
            ws_url,
            generated_client,
            chain_id,
            chain_hash,
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
        Self::builder().url(MAINNET_URL).build().await
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
