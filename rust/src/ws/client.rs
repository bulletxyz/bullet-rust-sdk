//! WebSocket client for real-time market data and order updates.
//!
//! This module provides a WebSocket client for connecting to the trading API's
//! real-time data streams.
//!
//! # Features
//!
//! - **Protocol-level keepalive**: The server handles keepalive via WebSocket
//!   protocol-level ping/pong frames (managed automatically by the transport).
//! - **Cross-platform**: Works on both native Rust and WASM targets.
//! - **Graceful error handling**: Parse failures return `ServerMessage::Unknown`
//!   with the error and raw message text for debugging.
//!
//! # Example
//!
//! ```no_run
//! use bullet_rust_sdk::Client;
//! use bullet_rust_sdk::errors::WSErrors;
//! use bullet_rust_sdk::types::ClientMessage;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let api = Client::mainnet().await?;
//!
//! 'reconnect: loop {
//!     let mut ws = api.connect_ws().call().await?;
//!
//!     ws.send(ClientMessage::Subscribe {
//!         id: Some(1.into()),
//!         params: vec!["BTC-USD@aggTrade".to_string()],
//!     }).await?;
//!
//!     loop {
//!         match ws.recv().await {
//!             Ok(msg) => println!("Received: {:?}", msg),
//!             Err(WSErrors::WsClosed { code, reason }) => {
//!                 eprintln!("Closed ({code:?}): {reason}");
//!                 continue 'reconnect;
//!             }
//!             Err(WSErrors::WsStreamEnded) => {
//!                 eprintln!("Connection lost");
//!                 continue 'reconnect;
//!             }
//!             Err(e) => return Err(e.into()),
//!         }
//!     }
//! }
//! # }
//! ```
//!
//! # Keepalive Behavior
//!
//! The server handles keepalive automatically using WebSocket protocol-level
//! ping/pong frames. No application-level pings are needed.

use std::ops::Deref;

use crate::types::{ClientMessage, OrderParams, RequestId};
use bon::{Builder, bon};
use futures::{FutureExt, SinkExt, StreamExt, select};
use futures_timer::Delay;
use tracing::{debug, warn};
use web_time::Duration;

use super::models::{ServerMessage, TaggedMessage};
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::{Client, SDKResult};

/// Default connection timeout in seconds.
const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 10;

/// Handle to an active WebSocket connection.
///
/// Provides methods to send messages and receive responses.
///
/// # Thread Safety
///
/// `WebsocketHandle` is `Send` but **not `Sync`**. The underlying WebSocket
/// transport contains non-thread-safe internal buffers.
///
/// If you need to share the handle across async tasks, wrap it in a
/// [`tokio::sync::Mutex`]:
///
/// ```ignore
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
///
/// let ws = Arc::new(Mutex::new(client.connect_ws().call().await?));
///
/// // Receiving task
/// let ws_recv = ws.clone();
/// tokio::spawn(async move {
///     loop {
///         let msg = ws_recv.lock().await.recv().await;
///         // handle msg ...
///     }
/// });
///
/// // Sending task
/// ws.lock().await.subscribe([Topic::agg_trade("BTC-USD")], None).await?;
/// ```
///
/// For high-throughput bots, a common pattern is to dedicate one task to the
/// WebSocket and use [`tokio::sync::mpsc`] channels to communicate with other
/// tasks — this avoids lock contention on the hot path.
///
/// # Extracting the Inner Socket
///
/// If you need direct access to the underlying `reqwest_websocket::WebSocket`,
/// use the `Deref` implementation.
pub struct WebsocketHandle {
    socket: reqwest_websocket::WebSocket,
}

impl WebsocketHandle {
    /// Connect to a WebSocket endpoint and wait for the server's handshake.
    ///
    /// Shared by `Client::connect_ws` and `ManagedWsClient::connect`.
    pub(crate) async fn connect(
        ws_client: &reqwest::Client,
        ws_url: &str,
        timeout: web_time::Duration,
    ) -> SDKResult<Self, WSErrors> {
        use reqwest_websocket::Upgrade;

        let response: reqwest_websocket::UpgradeResponse =
            ws_client.clone().get(ws_url).upgrade().send().await?;
        let websocket = response.into_websocket().await?;
        let mut handle = Self { socket: websocket };
        handle.wait_for_connected(timeout).await?;
        Ok(handle)
    }
}

/// Configuration for WebSocket connection behavior.
///
/// # Example
///
/// ```no_run
/// use bullet_rust_sdk::{Client, ws::WebsocketConfig};
/// use web_time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = Client::mainnet().await?;
///
/// // Use a longer connection timeout
/// let config = WebsocketConfig {
///     connection_timeout: Duration::from_secs(30),
/// };
/// let mut ws = api.connect_ws().config(config).call().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Builder, Clone, Debug)]
pub struct WebsocketConfig {
    /// How long to wait for the server's "connected" message during handshake.
    ///
    /// Default: 10 seconds
    #[builder(default = Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS))]
    pub connection_timeout: Duration,
}

impl Default for WebsocketConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Deref for WebsocketHandle {
    type Target = reqwest_websocket::WebSocket;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

#[bon]
impl Client {
    /// Open a raw WebSocket connection.
    ///
    /// For production bots, prefer [`connect_ws_managed`](Client::connect_ws_managed)
    /// which handles reconnection automatically.
    #[builder]
    pub async fn connect_ws(
        &self,
        config: Option<WebsocketConfig>,
    ) -> SDKResult<WebsocketHandle, WSErrors> {
        let config = config.unwrap_or_default();
        WebsocketHandle::connect(&self.ws_client, self.ws_url(), config.connection_timeout).await
    }
}

impl WebsocketHandle {
    /// Wait for the server's "connected" status message.
    ///
    /// Called automatically during connection. Times out if no message received
    /// within the specified timeout.
    async fn wait_for_connected(&mut self, timeout: Duration) -> SDKResult<(), WSErrors> {
        // Note: web_time::Duration is std::time::Duration on native, but different on WASM.
        // The try_into() is needed for WASM compatibility.
        #[allow(clippy::useless_conversion)]
        let timeout = Delay::new(timeout.try_into().unwrap_or(std::time::Duration::from_secs(
            DEFAULT_CONNECTION_TIMEOUT_SECS,
        )));

        debug!("Waiting for connected message from websocket.");

        select! {
            result = self.recv().fuse() => {
                match result? {
                    ServerMessage::Tagged(TaggedMessage::Status(status))
                        if status.status == "connected" =>
                    {
                        debug!("Successfully got connected message, continuing");
                        Ok(())
                    }
                    other => Err(WSErrors::WsHandshakeFailed(format!("{other:?}"))),
                }
            }
            _ = timeout.fuse() => {
                Err(WSErrors::WsConnectionTimeout)
            }
        }
    }

    /// Send a message to the server.
    ///
    /// # Available Message Types
    ///
    /// - `ClientMessage::Subscribe` - Subscribe to market data streams
    /// - `ClientMessage::Unsubscribe` - Unsubscribe from streams
    /// - `ClientMessage::ListSubscriptions` - List active subscriptions
    /// - `ClientMessage::Ping` - Manual ping (not needed for keepalive)
    /// - `ClientMessage::OrderPlace` - Place an order
    /// - `ClientMessage::OrderCancel` - Cancel an order
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::types::ClientMessage;
    /// # use bullet_rust_sdk::Client;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let api = Client::mainnet().await?;
    /// # let mut ws = api.connect_ws().call().await?;
    /// // Subscribe to aggregated trades
    /// ws.send(ClientMessage::Subscribe {
    ///     id: Some(1.into()),
    ///     params: vec!["BTC-USD@aggTrade".to_string()],
    /// }).await?;
    ///
    /// // Unsubscribe later
    /// ws.send(ClientMessage::Unsubscribe {
    ///     id: Some(2.into()),
    ///     params: vec!["BTC-USD@aggTrade".to_string()],
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&mut self, msg: ClientMessage) -> SDKResult<(), WSErrors> {
        let string_msg = serde_json::to_string(&msg)?;
        self.socket
            .send(reqwest_websocket::Message::Text(string_msg))
            .await?;
        Ok(())
    }

    /// Receive the next message from the server.
    ///
    /// # Errors
    ///
    /// - [`WSErrors::WsClosed`] - Server closed the connection (includes close code and reason)
    /// - [`WSErrors::WsStreamEnded`] - Connection ended unexpectedly without a close frame
    /// - [`WSErrors::WsUpgradeError`] - WebSocket protocol error
    ///
    /// # Parse Errors
    ///
    /// If a message cannot be parsed into a known [`ServerMessage`] variant,
    /// it returns `ServerMessage::Unknown(error, raw_text)` instead of failing.
    /// This allows you to log or debug unexpected message formats.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::ws::models::{ServerMessage, TaggedMessage};
    /// use bullet_rust_sdk::errors::WSErrors;
    /// use bullet_rust_sdk::types::ClientMessage;
    /// # use bullet_rust_sdk::Client;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    ///
    /// 'reconnect: loop {
    ///     let mut ws = api.connect_ws().call().await?;
    ///
    ///     ws.send(ClientMessage::Subscribe {
    ///         id: Some(1.into()),
    ///         params: vec!["BTC-USD@aggTrade".to_string()],
    ///     }).await?;
    ///
    ///     loop {
    ///         match ws.recv().await {
    ///             Ok(msg) => match msg {
    ///                 ServerMessage::AggTrade(trade) => {
    ///                     println!("Trade: {} @ {}", trade.symbol, trade.price);
    ///                 }
    ///                 ServerMessage::Tagged(TaggedMessage::Pong(_)) => {}
    ///                 ServerMessage::Tagged(TaggedMessage::Error(err)) => {
    ///                     eprintln!("Server error: {:?}", err);
    ///                 }
    ///                 _ => {}
    ///             },
    ///             Err(WSErrors::WsClosed { code, reason }) => {
    ///                 eprintln!("Connection closed (code {:?}): {}", code, reason);
    ///                 continue 'reconnect;
    ///             }
    ///             Err(WSErrors::WsStreamEnded) => {
    ///                 eprintln!("Connection lost unexpectedly");
    ///                 continue 'reconnect;
    ///             }
    ///             Err(e) => return Err(e.into()),
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn recv(&mut self) -> SDKResult<ServerMessage, WSErrors> {
        while let Some(msg) = self.socket.next().await {
            let msg = msg?;

            match msg {
                reqwest_websocket::Message::Text(text) => {
                    let server_msg = match serde_json::from_str::<ServerMessage>(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(?e, "Failed to parse ServerMessage, returning Unknown");
                            ServerMessage::Unknown(e.to_string(), text)
                        }
                    };
                    return Ok(server_msg);
                }
                reqwest_websocket::Message::Binary(data) => {
                    let text = String::from_utf8_lossy(&data).to_string();
                    let server_msg = match serde_json::from_slice::<ServerMessage>(&data) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(?e, "Failed to parse ServerMessage, returning Unknown");
                            ServerMessage::Unknown(e.to_string(), text)
                        }
                    };
                    return Ok(server_msg);
                }
                reqwest_websocket::Message::Close { code, reason } => {
                    return Err(WSErrors::WsClosed { code, reason });
                }
                _ => continue,
            }
        }

        Err(WSErrors::WsStreamEnded)
    }

    /// Subscribe to one or more topics.
    ///
    /// # Arguments
    ///
    /// * `topics` - Topics to subscribe to
    /// * `id` - Optional request ID for matching the server's response
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::{Client, Topic, OrderbookDepth};
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let mut ws = api.connect_ws().call().await?;
    ///
    /// // Subscribe to multiple topics using type-safe builders
    /// ws.subscribe([
    ///     Topic::agg_trade("BTC-USD"),
    ///     Topic::depth("ETH-USD", OrderbookDepth::D10),
    ///     Topic::book_ticker("SOL-USD"),
    /// ], Some(RequestId::new(1))).await?;
    ///
    /// // Now receive market data
    /// loop {
    ///     let msg = ws.recv().await?;
    ///     println!("{:?}", msg);
    /// }
    /// # }
    /// ```
    pub async fn subscribe(
        &mut self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::Subscribe {
            id,
            params: topics.into_iter().map(|t| t.to_string()).collect(),
        })
        .await
    }

    /// Unsubscribe from one or more topics.
    ///
    /// Unsubscribe is idempotent - unsubscribing from topics you're not
    /// subscribed to will still succeed.
    ///
    /// # Arguments
    ///
    /// * `topics` - Topics to unsubscribe from
    /// * `id` - Optional request ID for matching the server's response
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::Client;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let mut ws = api.connect_ws().call().await?;
    ///
    /// ws.list_subscriptions(Some(RequestId::new(1))).await?;
    /// // Match response by request_id
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_subscriptions(&mut self, id: Option<RequestId>) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::ListSubscriptions { id }).await
    }

    /// Place an order via WebSocket.
    ///
    /// # Arguments
    ///
    /// * `tx` - Base64-encoded raw transaction bytes
    /// * `id` - Optional request ID for matching the server's response
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::Client;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let mut ws = api.connect_ws().call().await?;
    ///
    /// let tx_bytes = "base64_encoded_transaction";
    /// ws.order_place(tx_bytes, Some(RequestId::new(1))).await?;
    /// // Match response by request_id
    /// # Ok(())
    /// # }
    /// ```
    pub async fn order_place(
        &mut self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::OrderPlace {
            id,
            params: OrderParams { tx: tx.into() },
        })
        .await
    }

    /// Cancel an order via WebSocket.
    ///
    /// # Arguments
    ///
    /// * `tx` - Base64-encoded raw transaction bytes
    /// * `id` - Optional request ID for matching the server's response
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::Client;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = Client::mainnet().await?;
    /// let mut ws = api.connect_ws().call().await?;
    ///
    /// let tx_bytes = "base64_encoded_cancel_transaction";
    /// ws.order_cancel(tx_bytes, Some(RequestId::new(1))).await?;
    /// // Match response by request_id
    /// # Ok(())
    /// # }
    /// ```
    pub async fn order_cancel(
        &mut self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::OrderCancel {
            id,
            params: OrderParams { tx: tx.into() },
        })
        .await
    }

    /// Place an order via WebSocket using a signed transaction.
    ///
    /// This is a convenience wrapper around [`order_place`](Self::order_place) that
    /// handles base64 encoding internally.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bullet_rust_sdk::{Client, Transaction};
    ///
    /// let signed = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .client(&client)
    ///     .build()?;
    ///
    /// ws.place_order(&signed, None).await?;
    /// ```
    pub async fn place_order(
        &mut self,
        signed: &bullet_exchange_interface::transaction::Transaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 = crate::Transaction::to_base64(signed)
            .map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_place(base64, id).await
    }

    /// Cancel an order via WebSocket using a signed transaction.
    ///
    /// This is a convenience wrapper around [`order_cancel`](Self::order_cancel) that
    /// handles base64 encoding internally.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use bullet_rust_sdk::{Client, Transaction};
    ///
    /// let signed = Transaction::builder()
    ///     .call_message(cancel_msg)
    ///     .client(&client)
    ///     .build()?;
    ///
    /// ws.cancel_order(&signed, None).await?;
    /// ```
    pub async fn cancel_order(
        &mut self,
        signed: &bullet_exchange_interface::transaction::Transaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 = crate::Transaction::to_base64(signed)
            .map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel(base64, id).await
    }
}
