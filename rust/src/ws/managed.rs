//! Auto-reconnecting WebSocket client.
//!
//! [`ManagedWebsocket`] wraps the raw [`WebsocketHandle`](super::client::WebsocketHandle)
//! in a background task that handles reconnection with exponential backoff and
//! replays subscriptions after each reconnect.
//!
//! Unlike `WebsocketHandle`, `ManagedWebsocket` is `Send + Sync` — it communicates
//! with the background task via channels, so it can be shared across async tasks
//! without a `Mutex`.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::{Client, Topic, OrderbookDepth};
//! use bullet_rust_sdk::ws::managed::ManagedWebsocket;
//!
//! let client = Client::mainnet().await?;
//! let mut ws = client.connect_ws_managed().call().await?;
//!
//! ws.subscribe([Topic::depth("BTC-USD", OrderbookDepth::D20)], None).await?;
//!
//! // Receive messages — reconnection is handled automatically
//! while let Some(event) = ws.recv().await {
//!     match event {
//!         WsEvent::Message(msg) => { /* process msg */ }
//!         WsEvent::Reconnecting => { /* log reconnect */ }
//!         WsEvent::Disconnected(err) => { /* permanent failure */ break; }
//!     }
//! }
//! ```

use std::time::Duration;

use bon::bon;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::client::WebsocketConfig;
#[allow(unused_imports)]
use super::models::ServerMessage;
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::types::{ClientMessage, OrderParams, RequestId};
use crate::Client;

/// Events delivered to the user from the managed WebSocket.
#[derive(Debug)]
pub enum WsEvent {
    /// A message from the server.
    Message(Box<ServerMessage>),
    /// The connection was lost and a reconnect is in progress.
    /// Subscriptions will be replayed automatically.
    Reconnecting,
    /// The connection was permanently lost after exhausting retries.
    Disconnected(String),
}

/// Configuration for managed WebSocket reconnection behavior.
#[derive(Clone)]
pub struct ManagedWsConfig {
    /// Initial delay before the first reconnect attempt.
    ///
    /// Default: 1 second
    pub initial_backoff: Duration,

    /// Maximum delay between reconnect attempts.
    ///
    /// Default: 30 seconds
    pub max_backoff: Duration,

    /// Maximum number of consecutive reconnect attempts before giving up.
    /// `None` means retry forever.
    ///
    /// Default: `None` (infinite retries)
    pub max_retries: Option<u32>,

    /// Underlying WebSocket connection config (e.g. handshake timeout).
    pub ws_config: Option<web_time::Duration>,
}

impl Default for ManagedWsConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            max_retries: None,
            ws_config: None,
        }
    }
}

/// Command sent from the user handle to the background task.
enum WsCommand {
    Subscribe(Vec<Topic>, Option<RequestId>),
    Unsubscribe(Vec<Topic>, Option<RequestId>),
    Send(ClientMessage),
    Stop,
}

/// Auto-reconnecting WebSocket handle.
///
/// `Send + Sync` — safe to share across async tasks.
pub struct ManagedWebsocket {
    event_rx: mpsc::UnboundedReceiver<WsEvent>,
    cmd_tx: mpsc::UnboundedSender<WsCommand>,
}

impl ManagedWebsocket {
    /// Receive the next event from the WebSocket.
    ///
    /// Returns `None` when the background task has stopped (after permanent
    /// disconnection or [`stop`](Self::stop)).
    pub async fn recv(&mut self) -> Option<WsEvent> {
        self.event_rx.recv().await
    }

    /// Subscribe to topics. The subscription is tracked and replayed on reconnect.
    pub async fn subscribe(
        &self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> Result<(), String> {
        let topics: Vec<Topic> = topics.into_iter().collect();
        self.cmd_tx
            .send(WsCommand::Subscribe(topics, id))
            .map_err(|_| "managed websocket stopped".to_string())
    }

    /// Unsubscribe from topics. Removes them from the replay list.
    pub async fn unsubscribe(
        &self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> Result<(), String> {
        let topics: Vec<Topic> = topics.into_iter().collect();
        self.cmd_tx
            .send(WsCommand::Unsubscribe(topics, id))
            .map_err(|_| "managed websocket stopped".to_string())
    }

    /// Place an order via WebSocket.
    pub async fn order_place(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::Send(ClientMessage::OrderPlace {
                id,
                params: OrderParams { tx: tx.into() },
            }))
            .map_err(|_| "managed websocket stopped".to_string())
    }

    /// Cancel an order via WebSocket.
    pub async fn order_cancel(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::Send(ClientMessage::OrderCancel {
                id,
                params: OrderParams { tx: tx.into() },
            }))
            .map_err(|_| "managed websocket stopped".to_string())
    }

    /// Gracefully stop the managed WebSocket and its background task.
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(WsCommand::Stop);
    }
}

impl Drop for ManagedWebsocket {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(WsCommand::Stop);
    }
}

#[bon]
impl Client {
    /// Open a managed (auto-reconnecting) WebSocket connection.
    ///
    /// Returns a [`ManagedWebsocket`] that handles reconnection with exponential
    /// backoff and replays subscriptions automatically.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut ws = client.connect_ws_managed().call().await?;
    /// ws.subscribe([Topic::agg_trade("BTC-USD")], None).await?;
    ///
    /// while let Some(event) = ws.recv().await {
    ///     match event {
    ///         WsEvent::Message(msg) => println!("{msg:?}"),
    ///         WsEvent::Reconnecting => eprintln!("reconnecting..."),
    ///         WsEvent::Disconnected(reason) => {
    ///             eprintln!("permanently disconnected: {reason}");
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    #[builder]
    pub async fn connect_ws_managed(
        &self,
        config: Option<ManagedWsConfig>,
    ) -> Result<ManagedWebsocket, WSErrors> {
        let config = config.unwrap_or_default();

        // Build WS config from the managed config's timeout
        let ws_cfg = config.ws_config.map(|timeout| {
            WebsocketConfig::builder().connection_timeout(timeout).build()
        });

        // Establish initial connection
        let ws = self.connect_ws().maybe_config(ws_cfg).call().await?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        // Clone what the background task needs
        let client = self.clone_for_managed_ws();
        let config_clone = config.clone();

        tokio::spawn(async move {
            run_managed_ws(client, ws, config_clone, event_tx, cmd_rx).await;
        });

        Ok(ManagedWebsocket { event_rx, cmd_tx })
    }
}

impl Client {
    /// Create a lightweight clone with only what's needed for reconnection.
    fn clone_for_managed_ws(&self) -> ManagedWsClient {
        ManagedWsClient {
            ws_client: self.ws_client.clone(),
            ws_url: self.ws_url().to_string(),
        }
    }
}

/// Minimal client data needed by the background task for reconnection.
struct ManagedWsClient {
    ws_client: reqwest::Client,
    ws_url: String,
}

impl ManagedWsClient {
    async fn connect(
        &self,
        connection_timeout: Option<web_time::Duration>,
    ) -> Result<super::client::WebsocketHandle, WSErrors> {
        use futures::{FutureExt, select};
        use futures_timer::Delay;
        use reqwest_websocket::Upgrade;

        let response: reqwest_websocket::UpgradeResponse = self
            .ws_client
            .clone()
            .get(&self.ws_url)
            .upgrade()
            .send()
            .await?;

        let websocket = response.into_websocket().await?;
        let mut handle = super::client::WebsocketHandle::new(websocket);

        let timeout = connection_timeout.unwrap_or(web_time::Duration::from_secs(10));

        // Wait for connected message
        #[allow(clippy::useless_conversion)]
        let std_timeout = timeout
            .try_into()
            .unwrap_or(std::time::Duration::from_secs(10));
        let delay = Delay::new(std_timeout);

        select! {
            result = handle.recv().fuse() => {
                match result? {
                    ServerMessage::Tagged(super::models::TaggedMessage::Status(status))
                        if status.status == "connected" =>
                    {
                        Ok(handle)
                    }
                    other => Err(WSErrors::WsHandshakeFailed(format!("{other:?}"))),
                }
            }
            _ = delay.fuse() => {
                Err(WSErrors::WsConnectionTimeout)
            }
        }
    }
}

/// Background task that manages the WebSocket lifecycle.
async fn run_managed_ws(
    client: ManagedWsClient,
    mut ws: super::client::WebsocketHandle,
    config: ManagedWsConfig,
    event_tx: mpsc::UnboundedSender<WsEvent>,
    mut cmd_rx: mpsc::UnboundedReceiver<WsCommand>,
) {
    // Track active subscriptions for replay
    let mut active_topics: Vec<String> = Vec::new();

    loop {
        tokio::select! {
            // Receive from WebSocket
            result = ws.recv() => {
                match result {
                    Ok(msg) => {
                        if event_tx.send(WsEvent::Message(Box::new(msg))).is_err() {
                            debug!("event receiver dropped, stopping managed ws");
                            return;
                        }
                    }
                    Err(WSErrors::WsClosed { code, reason }) => {
                        warn!(?code, %reason, "WebSocket closed, reconnecting");
                        if event_tx.send(WsEvent::Reconnecting).is_err() {
                            return;
                        }
                        match reconnect(&client, &config, &active_topics).await {
                            Ok(new_ws) => {
                                ws = new_ws;
                                info!("reconnected successfully");
                            }
                            Err(reason) => {
                                let _ = event_tx.send(WsEvent::Disconnected(reason));
                                return;
                            }
                        }
                    }
                    Err(WSErrors::WsStreamEnded) => {
                        warn!("WebSocket stream ended, reconnecting");
                        if event_tx.send(WsEvent::Reconnecting).is_err() {
                            return;
                        }
                        match reconnect(&client, &config, &active_topics).await {
                            Ok(new_ws) => {
                                ws = new_ws;
                                info!("reconnected successfully");
                            }
                            Err(reason) => {
                                let _ = event_tx.send(WsEvent::Disconnected(reason));
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!(?e, "WebSocket error");
                        let _ = event_tx.send(WsEvent::Disconnected(e.to_string()));
                        return;
                    }
                }
            }

            // Process commands from the user
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WsCommand::Subscribe(topics, id)) => {
                        let params: Vec<String> = topics.iter().map(|t| t.to_string()).collect();
                        // Track for replay
                        for p in &params {
                            if !active_topics.contains(p) {
                                active_topics.push(p.clone());
                            }
                        }
                        let _ = ws.send(ClientMessage::Subscribe {
                            id,
                            params,
                        }).await;
                    }
                    Some(WsCommand::Unsubscribe(topics, id)) => {
                        let params: Vec<String> = topics.iter().map(|t| t.to_string()).collect();
                        active_topics.retain(|t| !params.contains(t));
                        let _ = ws.send(ClientMessage::Unsubscribe {
                            id,
                            params,
                        }).await;
                    }
                    Some(WsCommand::Send(msg)) => {
                        let _ = ws.send(msg).await;
                    }
                    Some(WsCommand::Stop) | None => {
                        debug!("stopping managed ws");
                        return;
                    }
                }
            }
        }
    }
}

/// Reconnect with exponential backoff and replay subscriptions.
async fn reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &[String],
) -> Result<super::client::WebsocketHandle, String> {
    let mut backoff = config.initial_backoff;
    let mut attempts = 0u32;

    loop {
        attempts += 1;

        if let Some(max) = config.max_retries
            && attempts > max
        {
            return Err(format!("exhausted {max} reconnect attempts"));
        }

        info!(attempt = attempts, delay = ?backoff, "attempting reconnect");
        tokio::time::sleep(backoff).await;

        match client.connect(config.ws_config).await {
            Ok(mut ws) => {
                // Replay subscriptions
                if !active_topics.is_empty() {
                    debug!(count = active_topics.len(), "replaying subscriptions");
                    if let Err(e) = ws
                        .send(ClientMessage::Subscribe {
                            id: None,
                            params: active_topics.to_vec(),
                        })
                        .await
                    {
                        warn!(?e, "failed to replay subscriptions, retrying");
                        // Backoff and retry
                        backoff = (backoff * 2).min(config.max_backoff);
                        continue;
                    }
                }
                return Ok(ws);
            }
            Err(e) => {
                warn!(?e, attempt = attempts, "reconnect failed");
                backoff = (backoff * 2).min(config.max_backoff);
            }
        }
    }
}
