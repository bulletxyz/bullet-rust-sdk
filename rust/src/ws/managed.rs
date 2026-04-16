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
//! use bullet_rust_sdk::ws::managed::{ManagedWebsocket, WsEvent};
//!
//! let client = Client::mainnet().await?;
//! let mut ws = client.connect_ws_managed().call().await?;
//!
//! ws.subscribe([Topic::depth("BTC-USD", OrderbookDepth::D20)], None)?;
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

use std::collections::HashSet;
use std::time::Duration;

use bon::bon;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::client::WebsocketConfig;
use super::models::ServerMessage;
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::types::{ClientMessage, OrderParams, RequestId};
use crate::Client;

/// Errors from [`ManagedWebsocket`] operations.
#[derive(Debug, Error)]
pub enum ManagedWsError {
    /// The background task has stopped (disconnected or explicitly stopped).
    #[error("managed websocket is stopped")]
    Stopped,
}

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
///
/// # Example
///
/// ```ignore
/// use bullet_rust_sdk::ManagedWsConfig;
/// use std::time::Duration;
///
/// let config = ManagedWsConfig::builder()
///     .max_retries(10)
///     .initial_backoff(Duration::from_millis(500))
///     .build();
/// ```
#[derive(bon::Builder, Clone, Debug)]
pub struct ManagedWsConfig {
    /// Initial delay before the first reconnect attempt.
    ///
    /// Default: 1 second
    #[builder(default = Duration::from_secs(1))]
    pub initial_backoff: Duration,

    /// Maximum delay between reconnect attempts.
    ///
    /// Default: 30 seconds
    #[builder(default = Duration::from_secs(30))]
    pub max_backoff: Duration,

    /// Maximum number of consecutive reconnect attempts before giving up.
    /// `None` means retry forever.
    ///
    /// Default: `None` (infinite retries)
    pub max_retries: Option<u32>,

    /// Event channel buffer size. When the buffer is full and the consumer
    /// isn't keeping up, new events are dropped to keep the WebSocket
    /// connection alive. A warning is logged when this happens.
    ///
    /// Default: 10_000
    #[builder(default = 10_000)]
    pub channel_capacity: usize,

    /// Underlying WebSocket connection config (e.g. handshake timeout).
    pub ws_config: Option<WebsocketConfig>,
}

impl Default for ManagedWsConfig {
    fn default() -> Self {
        Self::builder().build()
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
/// `Send + Sync` — safe to share across async tasks without a `Mutex`.
///
/// Subscribe/unsubscribe are fire-and-forget (synchronous sends to the
/// background task). Server acknowledgements arrive as [`WsEvent::Message`]
/// on the event stream, matching the standard CEX WebSocket convention.
pub struct ManagedWebsocket {
    event_rx: mpsc::Receiver<WsEvent>,
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
    ///
    /// This is fire-and-forget — it queues the command to the background task.
    /// The server's subscribe acknowledgement arrives as a [`WsEvent::Message`].
    pub fn subscribe(
        &self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let topics: Vec<Topic> = topics.into_iter().collect();
        self.cmd_tx
            .send(WsCommand::Subscribe(topics, id))
            .map_err(|_| ManagedWsError::Stopped)
    }

    /// Unsubscribe from topics. Removes them from the replay list.
    pub fn unsubscribe(
        &self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let topics: Vec<Topic> = topics.into_iter().collect();
        self.cmd_tx
            .send(WsCommand::Unsubscribe(topics, id))
            .map_err(|_| ManagedWsError::Stopped)
    }

    /// Place an order via WebSocket.
    pub fn order_place(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.cmd_tx
            .send(WsCommand::Send(ClientMessage::OrderPlace {
                id,
                params: OrderParams { tx: tx.into() },
            }))
            .map_err(|_| ManagedWsError::Stopped)
    }

    /// Cancel an order via WebSocket.
    pub fn order_cancel(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.cmd_tx
            .send(WsCommand::Send(ClientMessage::OrderCancel {
                id,
                params: OrderParams { tx: tx.into() },
            }))
            .map_err(|_| ManagedWsError::Stopped)
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
    /// ws.subscribe([Topic::agg_trade("BTC-USD")], None)?;
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

        // Establish initial connection
        let ws = self
            .connect_ws()
            .maybe_config(config.ws_config.clone())
            .call()
            .await?;

        let (event_tx, event_rx) = mpsc::channel(config.channel_capacity);
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

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
        ws_config: &Option<WebsocketConfig>,
    ) -> Result<super::client::WebsocketHandle, WSErrors> {
        let timeout = ws_config
            .as_ref()
            .map(|c| c.connection_timeout)
            .unwrap_or(web_time::Duration::from_secs(10));
        super::client::WebsocketHandle::connect(&self.ws_client, &self.ws_url, timeout).await
    }
}

/// Background task that manages the WebSocket lifecycle.
async fn run_managed_ws(
    client: ManagedWsClient,
    mut ws: super::client::WebsocketHandle,
    config: ManagedWsConfig,
    event_tx: mpsc::Sender<WsEvent>,
    mut cmd_rx: mpsc::UnboundedReceiver<WsCommand>,
) {
    let mut active_topics: HashSet<String> = HashSet::new();

    loop {
        tokio::select! {
            result = ws.recv() => {
                match result {
                    Ok(msg) => {
                        match event_tx.try_send(WsEvent::Message(Box::new(msg))) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                warn!("event channel full, dropping message — consumer too slow");
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                debug!("event receiver dropped, stopping managed ws");
                                return;
                            }
                        }
                    }
                    Err(WSErrors::WsClosed { code, reason }) => {
                        warn!(?code, %reason, "WebSocket disconnected, reconnecting");
                        if handle_reconnect(&client, &config, &active_topics, &event_tx, &mut ws).await {
                            return;
                        }
                    }
                    Err(WSErrors::WsStreamEnded) => {
                        warn!("WebSocket disconnected, reconnecting");
                        if handle_reconnect(&client, &config, &active_topics, &event_tx, &mut ws).await {
                            return;
                        }
                    }
                    Err(e) => {
                        // Transport errors (WsUpgradeError, etc.) are transient —
                        // reconnect instead of permanently disconnecting.
                        warn!(?e, "WebSocket error, reconnecting");
                        if handle_reconnect(&client, &config, &active_topics, &event_tx, &mut ws).await {
                            return;
                        }
                    }
                }
            }

            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WsCommand::Subscribe(topics, id)) => {
                        let params: Vec<String> = topics.iter().map(|t| t.to_string()).collect();
                        for p in &params {
                            active_topics.insert(p.clone());
                        }
                        let _ = ws.send(ClientMessage::Subscribe { id, params }).await;
                    }
                    Some(WsCommand::Unsubscribe(topics, id)) => {
                        let params: Vec<String> = topics.iter().map(|t| t.to_string()).collect();
                        for p in &params {
                            active_topics.remove(p);
                        }
                        let _ = ws.send(ClientMessage::Unsubscribe { id, params }).await;
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

/// Handle reconnection. Returns `true` if the task should stop.
async fn handle_reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mpsc::Sender<WsEvent>,
    ws: &mut super::client::WebsocketHandle,
) -> bool {
    // Use try_send — if the consumer is stuck (which is likely during a
    // disconnect), we don't want to block the reconnect attempt.
    match event_tx.try_send(WsEvent::Reconnecting) {
        Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
        Err(mpsc::error::TrySendError::Closed(_)) => return true,
    }
    match reconnect(client, config, active_topics, event_tx).await {
        Ok(new_ws) => {
            *ws = new_ws;
            info!("reconnected successfully");
            false
        }
        Err(reason) => {
            // Best-effort notify; if channel is closed, we're stopping anyway.
            let _ = event_tx.try_send(WsEvent::Disconnected(reason));
            true
        }
    }
}

/// Reconnect with exponential backoff + jitter and replay subscriptions.
///
/// Checks `event_tx.is_closed()` each iteration so the loop stops promptly
/// when the `ManagedWebsocket` handle is dropped.
async fn reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mpsc::Sender<WsEvent>,
) -> Result<super::client::WebsocketHandle, String> {
    let mut backoff = config.initial_backoff;
    let mut attempts = 0u32;

    loop {
        // Stop if the user handle was dropped.
        if event_tx.is_closed() {
            return Err("handle dropped".to_string());
        }

        attempts += 1;

        if let Some(max) = config.max_retries
            && attempts > max
        {
            return Err(format!("exhausted {max} reconnect attempts"));
        }

        // Jitter: add 0..50% of backoff to avoid thundering herd
        let jitter_ms = rand::random::<u64>() % (backoff.as_millis() as u64 / 2 + 1);
        let jitter = Duration::from_millis(jitter_ms);
        let delay = backoff + jitter;

        info!(attempt = attempts, delay = ?delay, "attempting reconnect");
        tokio::time::sleep(delay).await;

        match client.connect(&config.ws_config).await {
            Ok(mut ws) => {
                if !active_topics.is_empty() {
                    let params: Vec<String> = active_topics.iter().cloned().collect();
                    debug!(count = params.len(), "replaying subscriptions");
                    if let Err(e) = ws
                        .send(ClientMessage::Subscribe {
                            id: None,
                            params,
                        })
                        .await
                    {
                        warn!(?e, "failed to replay subscriptions, retrying");
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
