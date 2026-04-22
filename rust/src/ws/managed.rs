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
//! This module is portable across native and wasm32 targets: the background
//! task is spawned via [`tokio::spawn`] on native and
//! [`wasm_bindgen_futures::spawn_local`] on wasm, and all time/channel
//! primitives come from the `futures` crate.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::{Client, Topic, OrderbookDepth};
//! use bullet_rust_sdk::ws::managed::{ManagedWebsocket, WsEvent};
//!
//! let client = Client::mainnet().await?;
//! let mut ws = ManagedWebsocket::connect(&client).call().await?;
//!
//! ws.subscribe([Topic::depth("BTC-USD", OrderbookDepth::D20)], None)?;
//!
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
use futures::channel::{mpsc, oneshot};
use futures::future::{self, Either};
use futures::{FutureExt, StreamExt};
use futures_timer::Delay;
use thiserror::Error;
use tracing::{debug, info, warn};

use super::client::{WebsocketConfig, WebsocketHandle};
use super::models::ServerMessage;
use super::topics::Topic;
use crate::Client;
use crate::errors::WSErrors;
use crate::types::{ClientMessage, OrderParams, RequestId};

/// Errors from [`ManagedWebsocket`] operations.
#[derive(Debug, Error)]
pub enum ManagedWsError {
    /// The background task has stopped (disconnected or the handle was dropped).
    #[error("managed websocket is stopped")]
    Stopped,
    /// The command channel is full — the background task is not draining fast
    /// enough. Indicates a stuck task or a pathological caller; treat as
    /// backpressure.
    #[error("managed websocket command channel is full")]
    Busy,
}

/// Why a reconnect attempt gave up.
#[derive(Debug, Error)]
enum ReconnectError {
    /// The user-facing handle was dropped while reconnecting.
    #[error("managed websocket handle dropped")]
    HandleDropped,
    /// Ran out of retry attempts.
    #[error("exhausted {0} reconnect attempts")]
    RetriesExhausted(u32),
    /// Subscription replay failed after reconnect with a non-transport error.
    /// The underlying [`WSErrors`] is preserved so callers can distinguish
    /// transient network failures from protocol-level problems (bad topic,
    /// too many topics).
    #[error("subscription replay failed: {0}")]
    ReplayFailed(#[source] WSErrors),
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

/// Minimum backoff floor. A zero `initial_backoff` would otherwise make
/// `backoff * 2` stay zero forever, producing a tight reconnect spin loop.
const MIN_BACKOFF: Duration = Duration::from_millis(10);

/// Default command channel capacity. Commands (subscribe, unsubscribe, order
/// send) are intrinsically low-rate; if you're queueing more than this, the
/// background task is stuck and the right answer is to surface
/// [`ManagedWsError::Busy`] rather than silently buffer.
const CMD_CHANNEL_CAPACITY: usize = 256;

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
///
/// Subscribe/unsubscribe commands carry already-serialized topic strings so
/// the background task doesn't need to re-serialize, and callers that already
/// have string topics (notably the WASM bindings) don't need to round-trip
/// through a typed [`Topic`].
enum WsCommand {
    Subscribe(Vec<String>, Option<RequestId>),
    Unsubscribe(Vec<String>, Option<RequestId>),
    Send(ClientMessage),
}

/// Auto-reconnecting WebSocket handle.
///
/// `Send + Sync` — safe to share across async tasks without a `Mutex`.
///
/// Subscribe/unsubscribe/order sends are fire-and-forget: the call queues a
/// command to the background task and returns. Server acknowledgements arrive
/// as [`WsEvent::Message`] on the event stream, matching standard CEX WS
/// conventions.
///
/// Dropping the handle (or calling [`stop`](Self::stop)) terminates the
/// background task immediately — even if it is mid-reconnect — via a separate
/// cancellation signal that bypasses the command queue.
pub struct ManagedWebsocket {
    event_rx: mpsc::Receiver<WsEvent>,
    cmd_tx: mpsc::Sender<WsCommand>,
    /// Held, never sent on. Dropping signals shutdown to the background task.
    _shutdown_tx: oneshot::Sender<()>,
}

impl ManagedWebsocket {
    /// Open a managed (auto-reconnecting) WebSocket connection for the given
    /// [`Client`].
    ///
    /// The returned handle manages reconnection with exponential backoff and
    /// replays subscriptions automatically.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut ws = ManagedWebsocket::connect(&client).call().await?;
    /// ws.subscribe([Topic::agg_trade("BTC-USD")], None)?;
    /// ```
    #[cfg_attr(not(target_arch = "wasm32"), doc = "Uses [`tokio::spawn`] on native targets.")]
    #[cfg_attr(
        target_arch = "wasm32",
        doc = "Uses [`wasm_bindgen_futures::spawn_local`] on wasm targets."
    )]
    pub async fn connect(client: &Client) -> Result<ManagedWebsocket, WSErrors> {
        Self::connect_with(client, ManagedWsConfig::default()).await
    }

    /// Like [`connect`](Self::connect) but takes an explicit [`ManagedWsConfig`].
    pub async fn connect_with(
        client: &Client,
        config: ManagedWsConfig,
    ) -> Result<ManagedWebsocket, WSErrors> {

        let ws = client
            .connect_ws()
            .maybe_config(config.ws_config.clone())
            .call()
            .await?;

        let (event_tx, event_rx) = mpsc::channel(config.channel_capacity);
        let (cmd_tx, cmd_rx) = mpsc::channel(CMD_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let inner = ManagedWsClient::from_client(client);

        spawn(async move {
            run_managed_ws(inner, ws, config, event_tx, cmd_rx, shutdown_rx).await;
        });

        Ok(ManagedWebsocket {
            event_rx,
            cmd_tx,
            _shutdown_tx: shutdown_tx,
        })
    }

    /// Receive the next event from the WebSocket.
    ///
    /// Returns `None` when the background task has stopped (after permanent
    /// disconnection or [`stop`](Self::stop)).
    pub async fn recv(&mut self) -> Option<WsEvent> {
        self.event_rx.next().await
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
        let params: Vec<String> = topics.into_iter().map(|t| t.to_string()).collect();
        self.try_send_cmd(WsCommand::Subscribe(params, id))
    }

    /// Subscribe using pre-serialized topic strings (e.g. `"BTC-USD@aggTrade"`).
    ///
    /// Prefer [`subscribe`](Self::subscribe) with typed [`Topic`] values from
    /// native Rust — this overload exists for binding layers (WASM/JS) that
    /// already hold string topics.
    pub fn subscribe_raw(
        &self,
        topics: impl IntoIterator<Item = String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let params: Vec<String> = topics.into_iter().collect();
        self.try_send_cmd(WsCommand::Subscribe(params, id))
    }

    /// Unsubscribe from topics. Removes them from the replay list.
    pub fn unsubscribe(
        &self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let params: Vec<String> = topics.into_iter().map(|t| t.to_string()).collect();
        self.try_send_cmd(WsCommand::Unsubscribe(params, id))
    }

    /// Raw-string counterpart of [`unsubscribe`](Self::unsubscribe).
    pub fn unsubscribe_raw(
        &self,
        topics: impl IntoIterator<Item = String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let params: Vec<String> = topics.into_iter().collect();
        self.try_send_cmd(WsCommand::Unsubscribe(params, id))
    }

    /// Place an order via WebSocket.
    pub fn order_place(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.try_send_cmd(WsCommand::Send(ClientMessage::OrderPlace {
            id,
            params: OrderParams { tx: tx.into() },
        }))
    }

    /// Cancel an order via WebSocket.
    pub fn order_cancel(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.try_send_cmd(WsCommand::Send(ClientMessage::OrderCancel {
            id,
            params: OrderParams { tx: tx.into() },
        }))
    }

    /// Place an order using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// Returns a `SDKResult`-style error instead of `ManagedWsError` because
    /// encoding can fail independently of the channel state.
    ///
    /// [`Transaction`]: bullet_exchange_interface::transaction::Transaction
    pub fn place_order(
        &self,
        signed: &bullet_exchange_interface::transaction::Transaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 = crate::Transaction::to_base64(signed)
            .map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_place(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Cancel an order using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// [`Transaction`]: bullet_exchange_interface::transaction::Transaction
    pub fn cancel_order(
        &self,
        signed: &bullet_exchange_interface::transaction::Transaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 = crate::Transaction::to_base64(signed)
            .map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Stop the managed WebSocket and its background task.
    ///
    /// After this returns the background task has been signaled; it will
    /// terminate at its next await point without draining pending commands.
    /// The event stream will end (`recv()` returns `None`) shortly after.
    pub fn stop(self) {
        // Drop self — `_shutdown_tx` is dropped, closing the oneshot. Task sees
        // the signal and exits without going through the cmd queue.
    }

    fn try_send_cmd(&self, cmd: WsCommand) -> Result<(), ManagedWsError> {
        // `Sender::clone` is an Arc bump, so try_send (which needs &mut self)
        // can be called without requiring `&mut self` on the public API.
        let mut tx = self.cmd_tx.clone();
        tx.try_send(cmd).map_err(|e| {
            if e.is_full() {
                ManagedWsError::Busy
            } else {
                ManagedWsError::Stopped
            }
        })
    }
}

// `mpsc::Sender`/`Receiver` are `Send`, and `oneshot::Sender<()>` is `Send + Sync`.
// The handle is explicitly `Send + Sync` on all targets so callers can share it
// across async tasks without a `Mutex`.

/// Convenience wrapper on [`Client`] that forwards to [`ManagedWebsocket::connect`].
///
/// Kept as a thin helper so the common case (`client.connect_ws_managed()`) is
/// discoverable; the real dependency still flows `managed → client`.
#[bon]
impl Client {
    #[builder]
    pub async fn connect_ws_managed(
        &self,
        config: Option<ManagedWsConfig>,
    ) -> Result<ManagedWebsocket, WSErrors> {
        match config {
            Some(c) => ManagedWebsocket::connect_with(self, c).await,
            None => ManagedWebsocket::connect(self).await,
        }
    }
}

/// Minimal client data needed by the background task for reconnection.
///
/// Constructed via [`ManagedWsClient::from_client`] so `Client` has no
/// compile-time dependency on this struct.
struct ManagedWsClient {
    ws_client: reqwest::Client,
    ws_url: String,
}

impl ManagedWsClient {
    fn from_client(client: &Client) -> Self {
        Self {
            ws_client: client.ws_client.clone(),
            ws_url: client.ws_url().to_string(),
        }
    }

    async fn connect(&self, ws_config: &Option<WebsocketConfig>) -> Result<WebsocketHandle, WSErrors> {
        let timeout = ws_config
            .as_ref()
            .map(|c| c.connection_timeout)
            .unwrap_or(web_time::Duration::from_secs(10));
        WebsocketHandle::connect(&self.ws_client, &self.ws_url, timeout).await
    }
}

/// Spawn a background future on the target's executor.
///
/// Native uses [`tokio::spawn`] (requires `Send`); wasm uses
/// [`wasm_bindgen_futures::spawn_local`] (no `Send` required since JS is
/// single-threaded).
#[cfg(not(target_arch = "wasm32"))]
fn spawn<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(fut);
}

#[cfg(target_arch = "wasm32")]
fn spawn<F>(fut: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(fut);
}

/// Background task that manages the WebSocket lifecycle.
async fn run_managed_ws(
    client: ManagedWsClient,
    mut ws: WebsocketHandle,
    config: ManagedWsConfig,
    mut event_tx: mpsc::Sender<WsEvent>,
    mut cmd_rx: mpsc::Receiver<WsCommand>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut active_topics: HashSet<String> = HashSet::new();

    /// One completed branch of the per-iteration select.
    ///
    /// `Recv` boxes the server message so this enum stays small — `ServerMessage`
    /// is ~340 bytes and sits on the stack every iteration otherwise.
    enum Branch {
        Shutdown,
        Recv(Result<Box<ServerMessage>, WSErrors>),
        Cmd(Option<WsCommand>),
    }

    loop {
        // Run the select in its own scope so the fused recv/cmd futures (which
        // hold `&mut ws` / `&mut cmd_rx`) are dropped before we touch those
        // receivers again in the match arms below. This mirrors what
        // `tokio::select!` does internally but has to be explicit with
        // `futures::select!` + `pin_mut!`.
        //
        // `WebsocketHandle::recv` and `cmd_rx.next()` are both cancel-safe
        // (stream polls), so re-creating them each iteration is fine.
        let branch = {
            let recv_fut = ws.recv().fuse();
            let cmd_fut = cmd_rx.next().fuse();
            futures::pin_mut!(recv_fut, cmd_fut);

            futures::select! {
                _ = (&mut shutdown_rx).fuse() => Branch::Shutdown,
                r = recv_fut => Branch::Recv(r.map(Box::new)),
                c = cmd_fut => Branch::Cmd(c),
            }
        };

        match branch {
            Branch::Shutdown => {
                debug!("shutdown signaled, stopping managed ws");
                return;
            }
            Branch::Recv(Ok(msg)) => match event_tx.try_send(WsEvent::Message(msg)) {
                Ok(()) => {}
                Err(e) if e.is_full() => {
                    warn!("event channel full, dropping message — consumer too slow");
                }
                Err(_) => {
                    debug!("event receiver dropped, stopping managed ws");
                    return;
                }
            },
            Branch::Recv(Err(e)) => {
                match &e {
                    WSErrors::WsClosed { code, reason } => {
                        warn!(?code, %reason, "WebSocket disconnected, reconnecting");
                    }
                    WSErrors::WsStreamEnded => {
                        warn!("WebSocket stream ended, reconnecting");
                    }
                    _ => {
                        // Transport errors (WsUpgradeError, etc.) are transient.
                        warn!(?e, "WebSocket error, reconnecting");
                    }
                }
                if handle_reconnect(
                    &client,
                    &config,
                    &active_topics,
                    &mut event_tx,
                    &mut ws,
                    &mut shutdown_rx,
                )
                .await
                {
                    return;
                }
            }
            Branch::Cmd(Some(WsCommand::Subscribe(params, id))) => {
                for p in &params {
                    active_topics.insert(p.clone());
                }
                if let Err(e) = ws.send(ClientMessage::Subscribe { id, params }).await {
                    debug!(?e, "subscribe send failed, will replay after reconnect");
                }
            }
            Branch::Cmd(Some(WsCommand::Unsubscribe(params, id))) => {
                for p in &params {
                    active_topics.remove(p);
                }
                if let Err(e) = ws.send(ClientMessage::Unsubscribe { id, params }).await {
                    debug!(?e, "unsubscribe send failed");
                }
            }
            Branch::Cmd(Some(WsCommand::Send(msg))) => {
                if let Err(e) = ws.send(msg).await {
                    // Order place/cancel send failed — the connection is dead.
                    // Caller's order was NOT delivered; they will see
                    // WsEvent::Reconnecting and should resubmit.
                    warn!(?e, "failed to send order message, reconnecting");
                    if handle_reconnect(
                        &client,
                        &config,
                        &active_topics,
                        &mut event_tx,
                        &mut ws,
                        &mut shutdown_rx,
                    )
                    .await
                    {
                        return;
                    }
                }
            }
            Branch::Cmd(None) => {
                debug!("command channel closed, stopping managed ws");
                return;
            }
        }
    }
}

/// Handle reconnection. Returns `true` if the task should stop.
async fn handle_reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mut mpsc::Sender<WsEvent>,
    ws: &mut WebsocketHandle,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> bool {
    match event_tx.try_send(WsEvent::Reconnecting) {
        Ok(()) => {}
        Err(e) if e.is_full() => {}
        Err(_) => return true,
    }
    match reconnect(client, config, active_topics, event_tx, shutdown_rx).await {
        Ok(new_ws) => {
            *ws = new_ws;
            info!("reconnected successfully");
            false
        }
        Err(ReconnectError::HandleDropped) => true,
        Err(err) => {
            let _ = event_tx.try_send(WsEvent::Disconnected(err.to_string()));
            true
        }
    }
}

/// Reconnect with exponential backoff + jitter and replay subscriptions.
///
/// Observes `shutdown_rx` during every sleep and between connect attempts so
/// dropping the handle terminates the loop promptly.
async fn reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mpsc::Sender<WsEvent>,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Result<WebsocketHandle, ReconnectError> {
    // Floor so a misconfigured zero doesn't produce a tight spin loop.
    let mut backoff = config.initial_backoff.max(MIN_BACKOFF);
    let max_backoff = config.max_backoff.max(MIN_BACKOFF);
    let mut attempts = 0u32;

    loop {
        if shutdown_observed(shutdown_rx) || event_tx.is_closed() {
            return Err(ReconnectError::HandleDropped);
        }

        attempts += 1;
        if let Some(max) = config.max_retries
            && attempts > max
        {
            return Err(ReconnectError::RetriesExhausted(max));
        }

        // Jitter: add 0..50% of backoff to avoid thundering herd.
        let jitter_ms = rand::random::<u64>() % (backoff.as_millis() as u64 / 2 + 1);
        let delay = backoff + Duration::from_millis(jitter_ms);

        info!(attempt = attempts, delay = ?delay, "attempting reconnect");

        match future::select(Delay::new(delay), &mut *shutdown_rx).await {
            Either::Left(_) => {}
            Either::Right(_) => return Err(ReconnectError::HandleDropped),
        }

        let connect_fut = client.connect(&config.ws_config);
        let connect_result = match future::select(Box::pin(connect_fut), &mut *shutdown_rx).await {
            Either::Left((r, _)) => r,
            Either::Right(_) => return Err(ReconnectError::HandleDropped),
        };

        match connect_result {
            Ok(mut ws) => {
                if !active_topics.is_empty() {
                    let params: Vec<String> = active_topics.iter().cloned().collect();
                    debug!(count = params.len(), "replaying subscriptions");
                    if let Err(e) = ws.send(ClientMessage::Subscribe { id: None, params }).await {
                        // Distinguish protocol errors (bad topic, oversize) from
                        // transport errors (connection died mid-replay).
                        if matches!(&e, WSErrors::WsStreamEnded | WSErrors::WsClosed { .. }) {
                            warn!(?e, "replay send lost connection, retrying");
                            backoff = (backoff * 2).min(max_backoff);
                            continue;
                        }
                        return Err(ReconnectError::ReplayFailed(e));
                    }
                }
                return Ok(ws);
            }
            Err(e) => {
                warn!(?e, attempt = attempts, "reconnect failed");
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

/// Returns `true` if shutdown has been signaled (sender sent, dropped, or the
/// receiver is otherwise resolved).
fn shutdown_observed(rx: &mut oneshot::Receiver<()>) -> bool {
    !matches!(rx.try_recv(), Ok(None))
}
