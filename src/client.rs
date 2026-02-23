use std::ops::Deref;

use crate::generated::Client;
use crate::{SDKError, SDKResult};
use reqwest::Url;

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
/// use bullet_rust_sdk::{Network, TradingApi};
///
/// // Connect to REST API
/// let api = TradingApi::mainnet().await?;
///
/// // Query via REST
/// let info = api.exchange_info().await?;
/// ```
pub struct TradingApi {
    rest_url: String,
    ws_url: String,
    generated_client: Client,
    chain_id: u64,
    chain_hash: [u8; 32],
}

pub const MAINNET_URL: &str = "https://tradingapi.bullet.xyz";

impl TradingApi {
    /// Create a new TradingApi from an URL.
    pub async fn new(url: &str, reqwest_client: Option<reqwest::Client>) -> SDKResult<Self> {
        let parsed = Url::parse(url).map_err(|_| SDKError::InvalidNetworkUrl)?;

        let (rest_url, ws_url) = match parsed.scheme() {
            "https" => (url.to_string(), format!("wss://{}/ws", parsed.authority())),
            "http" => (url.to_string(), format!("ws://{}/ws", parsed.authority())),
            _ => return Err(SDKError::InvalidNetworkUrl),
        };
        let generated_client = match reqwest_client {
            Some(client) => Client::new_with_client(&rest_url, client),
            None => Client::new(&rest_url),
        };

        // fetch chain_id if not provided
        let constants = generated_client.constants().await?;
        let chain_id = u64::try_from(constants.chain_id).map_err(SDKError::ChainIdCastError)?;

        // fetch schema
        let schema = generated_client.schema().await?;
        // XXX validate schema

        let chain_hash_hex = schema
            .get("chain_hash")
            .and_then(|v| v.as_str())
            .ok_or(SDKError::InvalidSchemaResponse("chain_hash"))?;

        let chain_hash_bytes = hex::decode(chain_hash_hex.replace("0x", ""))
            .map_err(|e| SDKError::InvalidChainHash(e.to_string()))?;

        let chain_hash = chain_hash_bytes.try_into().map_err(|v: Vec<u8>| {
            SDKError::InvalidChainHash(format!("expected 32 bytes, got {}", v.len()))
        })?;

        Ok(Self {
            rest_url,
            ws_url,
            generated_client,
            chain_id,
            chain_hash,
        })
    }

    /// Connect to the mainnet environment.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::TradingApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let info = api.exchange_info().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn mainnet() -> SDKResult<Self> {
        Self::new(MAINNET_URL, None).await
    }

    /// Get a reference to the underlying generated client.
    ///
    /// Prefer using `Deref` (calling methods directly on `TradingApi`)
    /// instead of this method.
    pub fn client(&self) -> &Client {
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
/// let api = TradingApi::new(network);
/// let info = api.exchange_info().await?;
/// ```
impl Deref for TradingApi {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        self.client()
    }
}
