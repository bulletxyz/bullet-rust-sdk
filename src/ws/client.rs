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
//! use bullet_rust_sdk::TradingApi;
//! use bullet_rust_sdk::errors::WSErrors;
//! use bullet_rust_sdk::types::ClientMessage;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let api = TradingApi::mainnet().await?;
//!
//! 'reconnect: loop {
//!     let mut ws = api.connect_ws().await?;
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
use futures::{FutureExt, SinkExt, StreamExt, select};
use futures_timer::Delay;
use tracing::{debug, warn};
use web_time::Duration;

use super::models::{ServerMessage, TaggedMessage};
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::{SDKResult, TradingApi};

/// Default connection timeout in seconds.
const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 10;

/// Handle to an active WebSocket connection.
///
/// Provides methods to send messages and receive responses.
///
/// # Extracting the Inner Socket
///
/// If you need direct access to the underlying `reqwest_websocket::WebSocket`,
/// use the `Deref` implementation.
pub struct WebsocketHandle {
    socket: reqwest_websocket::WebSocket,
}

/// Configuration for WebSocket connection behavior.
///
/// # Example
///
/// ```no_run
/// use bullet_rust_sdk::{TradingApi, ws::WebsocketConfig};
/// use web_time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let api = TradingApi::mainnet().await?;
///
/// // Use a longer connection timeout
/// let config = WebsocketConfig {
///     connection_timeout: Duration::from_secs(30),
/// };
/// let mut ws = api.connect_ws_with_config(config).await?;
/// # Ok(())
/// # }
/// ```
pub struct WebsocketConfig {
    /// How long to wait for the server's "connected" message during handshake.
    ///
    /// Default: 10 seconds
    pub connection_timeout: Duration,
}

impl Default for WebsocketConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS),
        }
    }
}

impl Deref for WebsocketHandle {
    type Target = reqwest_websocket::WebSocket;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl TradingApi {
    /// Connect to the WebSocket API with default configuration.
    ///
    /// Returns a [`WebsocketHandle`] ready for sending and receiving messages.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::TradingApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
    /// // Connection is ready to use
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_ws(&self) -> SDKResult<WebsocketHandle, WSErrors> {
        self.connect_ws_with_config(WebsocketConfig::default())
            .await
    }

    /// Connect to the WebSocket API with custom configuration.
    ///
    /// Use this if you need to adjust the connection timeout.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::{TradingApi, ws::WebsocketConfig};
    /// use web_time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    ///
    /// let config = WebsocketConfig {
    ///     connection_timeout: Duration::from_secs(30),
    /// };
    /// let mut ws = api.connect_ws_with_config(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_ws_with_config(
        &self,
        config: WebsocketConfig,
    ) -> SDKResult<WebsocketHandle, WSErrors> {
        use reqwest_websocket::RequestBuilderExt;

        let response = self
            .client
            .clone()
            .get(self.ws_url())
            .upgrade()
            .send()
            .await?;

        let websocket = response.into_websocket().await?;

        let mut handle = WebsocketHandle { socket: websocket };

        // Wait for the server's "connected" status message with timeout
        handle.wait_for_connected(config.connection_timeout).await?;

        Ok(handle)
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
    /// # use bullet_rust_sdk::TradingApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let api = TradingApi::mainnet().await?;
    /// # let mut ws = api.connect_ws().await?;
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
    /// # use bullet_rust_sdk::TradingApi;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    ///
    /// 'reconnect: loop {
    ///     let mut ws = api.connect_ws().await?;
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
    /// use bullet_rust_sdk::{TradingApi, Topic, OrderbookDepth};
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
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
    /// use bullet_rust_sdk::{TradingApi, Topic};
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
    ///
    /// let topic = Topic::agg_trade("BTC-USD");
    /// ws.subscribe([topic.clone()], Some(RequestId::new(1))).await?;
    /// // ... receive some messages ...
    /// ws.unsubscribe([topic], Some(RequestId::new(2))).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unsubscribe(
        &mut self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::Unsubscribe {
            id,
            params: topics.into_iter().map(|t| t.to_string()).collect(),
        })
        .await
    }

    /// List all active subscriptions.
    ///
    /// # Arguments
    ///
    /// * `id` - Optional request ID for matching the server's response
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bullet_rust_sdk::TradingApi;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
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
    /// use bullet_rust_sdk::TradingApi;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
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
    /// use bullet_rust_sdk::TradingApi;
    /// use bullet_rust_sdk::types::RequestId;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let api = TradingApi::mainnet().await?;
    /// let mut ws = api.connect_ws().await?;
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
}
