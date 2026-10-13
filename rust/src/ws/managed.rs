//! WebSocket reconnection with backoff and subscription replay.
//! [`ManagedWebsocket`] owns the raw socket in a background task and is `Send + Sync`.

use std::collections::HashSet;
use std::time::Duration;

use bon::bon;
use futures::channel::{mpsc, oneshot};
use futures::future::{self, Either, pending};
use futures::{FutureExt, StreamExt};
use futures_timer::Delay;
use thiserror::Error;
use tracing::{debug, info, warn};
use web_time::Instant;

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
    /// The command channel is full: the background task is stuck or the caller is flooding it.
    #[error("managed websocket command channel is full")]
    Busy,
}

#[derive(Debug, Error)]
enum ReconnectError {
    #[error("managed websocket handle dropped")]
    HandleDropped,
    #[error("exhausted {0} reconnect attempts")]
    RetriesExhausted(u32),
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

// Floor so a zero `initial_backoff` can't double to zero forever and spin.
const MIN_BACKOFF: Duration = Duration::from_millis(10);

// Commands are low-rate; a full queue surfaces `ManagedWsError::Busy` rather than buffering.
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

    /// Event channel buffer size. When full, new events are dropped (with a warning) to keep the
    /// connection alive.
    /// Default: 10_000
    #[builder(default = 10_000)]
    pub channel_capacity: usize,

    /// Underlying WebSocket connection config (e.g. handshake timeout).
    pub ws_config: Option<WebsocketConfig>,

    /// Force a reconnect if no server message arrives within this window.
    ///
    /// Guards against zombie connections that stay open but stop sending. Any server message,
    /// including acks, resets the clock. `Duration::ZERO` disables it.
    ///
    /// Default: 60 seconds
    #[builder(default = Duration::from_secs(60))]
    pub idle_timeout: Duration,

    /// Uptime after which the next disconnect restarts from
    /// [`initial_backoff`](Self::initial_backoff). Without it, a server that accepts then drops
    /// would be retried at `initial_backoff` forever.
    ///
    /// Default: 30 seconds
    #[builder(default = Duration::from_secs(30))]
    pub backoff_reset_after: Duration,
}

impl Default for ManagedWsConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

// Serialized topics let WASM callers skip a typed `Topic` round-trip.
enum WsCommand {
    Subscribe(Vec<String>, Option<RequestId>),
    Unsubscribe(Vec<String>, Option<RequestId>),
    Send(ClientMessage),
}

/// Auto-reconnecting WebSocket handle.
///
/// `Send + Sync` — safe to share across async tasks without a `Mutex`.
///
/// Subscribe/unsubscribe/order calls are fire-and-forget; server acks arrive as
/// [`WsEvent::Message`]. Dropping the handle or calling [`stop`](Self::stop) ends the background
/// task immediately, even mid-reconnect.
pub struct ManagedWebsocket {
    event_rx: mpsc::Receiver<WsEvent>,
    cmd_tx: mpsc::Sender<WsCommand>,
    // Held, never sent on. Dropping signals shutdown to the background task.
    _shutdown_tx: oneshot::Sender<()>,
}

impl ManagedWebsocket {
    /// Connect with default backoff and automatic subscription replay.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        doc = "Uses [`tokio::spawn`] on native targets."
    )]
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
    /// Fire-and-forget; the server ack arrives as a [`WsEvent::Message`].
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
    /// For binding layers (WASM/JS) that already hold string topics; prefer
    /// [`subscribe`](Self::subscribe) from Rust.
    pub fn subscribe_raw(
        &self,
        topics: impl IntoIterator<Item = String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        let params: Vec<String> = topics.into_iter().collect();
        self.try_send_cmd(WsCommand::Subscribe(params, id))
    }

    /// Send an application-level ping.
    ///
    /// The server answers with a `Pong`, which arrives as a
    /// [`WsEvent::Message`] and — unlike a protocol-level ping/pong, which is
    /// handled inside the transport and never surfaces here — resets the
    /// [`idle_timeout`](ManagedWsConfig::idle_timeout) clock. On a connection
    /// whose subscriptions only speak when something changes, this is the
    /// only way to tell a quiet server from a wedged one.
    pub fn ping(&self, id: Option<RequestId>) -> Result<(), ManagedWsError> {
        self.try_send_cmd(WsCommand::Send(ClientMessage::Ping { id }))
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

    /// Amend an order via WebSocket.
    pub fn order_amend(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.try_send_cmd(WsCommand::Send(ClientMessage::OrderAmend {
            id,
            params: OrderParams { tx: tx.into() },
        }))
    }

    /// Cancel all open orders for a market via WebSocket.
    pub fn order_cancel_all(
        &self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> Result<(), ManagedWsError> {
        self.try_send_cmd(WsCommand::Send(ClientMessage::OrderCancelAll {
            id,
            params: OrderParams { tx: tx.into() },
        }))
    }

    /// Place an order using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// Returns `SDKResult` rather than `ManagedWsError` because encoding can fail too.
    ///
    /// [`Transaction`]: crate::SignedTransaction
    pub fn place_order(
        &self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_place(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Cancel an order using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// [`Transaction`]: crate::SignedTransaction
    pub fn cancel_order(
        &self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Amend an order using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// [`Transaction`]: crate::SignedTransaction
    pub fn amend_order(
        &self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_amend(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Cancel all open orders using a signed [`Transaction`]. Base64-encodes internally.
    ///
    /// [`Transaction`]: crate::SignedTransaction
    pub fn cancel_all_orders(
        &self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> Result<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel_all(base64, id)
            .map_err(|e| WSErrors::WsError(e.to_string()))
    }

    /// Stop the managed WebSocket and its background task.
    ///
    /// The task exits at its next await point without draining pending commands; `recv()` then
    /// returns `None`.
    pub fn stop(self) {
        // Dropping `_shutdown_tx` closes the oneshot, bypassing the cmd queue.
    }

    fn try_send_cmd(&self, cmd: WsCommand) -> Result<(), ManagedWsError> {
        // `Sender::clone` is an Arc bump; lets `try_send` (needs `&mut`) work from `&self`.
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

// Explicitly `Send + Sync` on all targets so callers can share it without a `Mutex`.

/// Convenience wrapper on [`Client`] that forwards to [`ManagedWebsocket::connect`].
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

    async fn connect(
        &self,
        ws_config: &Option<WebsocketConfig>,
    ) -> Result<WebsocketHandle, WSErrors> {
        let timeout = ws_config
            .as_ref()
            .map(|c| c.connection_timeout)
            .unwrap_or(web_time::Duration::from_secs(10));
        WebsocketHandle::connect(&self.ws_client, &self.ws_url, timeout).await
    }
}

/// Spawn a background future on the target's executor.
///
/// Native uses [`tokio::spawn`] (`Send`); wasm uses [`wasm_bindgen_futures::spawn_local`].
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

/// Persistent state carried across reconnect cycles.
///
/// Backoff persists across disconnects and resets only after a connection stays up for
/// `backoff_reset_after`, so accept-then-drop servers still get exponential relief.
struct ReconnectState {
    backoff: Duration,
    /// When the current connection was established; set by [`run_managed_ws`] on entry.
    connected_since: Option<Instant>,
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
    let mut last_msg = Instant::now();
    let mut state = ReconnectState {
        backoff: config.initial_backoff.max(MIN_BACKOFF),
        connected_since: Some(Instant::now()),
    };

    /// One completed branch of the per-iteration select.
    ///
    /// `Recv` is boxed: `ServerMessage` is ~340 bytes.
    enum Branch {
        Shutdown,
        Recv(Result<Box<ServerMessage>, WSErrors>),
        Cmd(Option<WsCommand>),
        Idle,
    }

    loop {
        let idle_remaining = if config.idle_timeout.is_zero() {
            None
        } else {
            Some(config.idle_timeout.saturating_sub(last_msg.elapsed()))
        };

        // Scoped so the fused futures borrowing `ws`/`cmd_rx` drop before the match arms.
        let branch = {
            let recv_fut = ws.recv().fuse();
            let cmd_fut = cmd_rx.next().fuse();
            let idle_fut = match idle_remaining {
                Some(d) => Either::Left(Delay::new(d)),
                None => Either::Right(pending::<()>()),
            }
            .fuse();
            futures::pin_mut!(recv_fut, cmd_fut, idle_fut);

            futures::select! {
                _ = (&mut shutdown_rx).fuse() => Branch::Shutdown,
                r = recv_fut => Branch::Recv(r.map(Box::new)),
                c = cmd_fut => Branch::Cmd(c),
                _ = idle_fut => Branch::Idle,
            }
        };

        match branch {
            Branch::Shutdown => {
                debug!("shutdown signaled, stopping managed ws");
                return;
            }
            Branch::Recv(Ok(msg)) => {
                // Server proved it's alive — reset the idle timer.
                last_msg = Instant::now();
                match event_tx.try_send(WsEvent::Message(msg)) {
                    Ok(()) => {}
                    Err(e) if e.is_full() => {
                        warn!("event channel full, dropping message — consumer too slow");
                    }
                    Err(_) => {
                        debug!("event receiver dropped, stopping managed ws");
                        return;
                    }
                }
            }
            Branch::Recv(Err(e)) => {
                match &e {
                    WSErrors::WsClosed { code, reason } => {
                        warn!(?code, %reason, "WebSocket disconnected, reconnecting");
                    }
                    WSErrors::WsStreamEnded => {
                        warn!("WebSocket stream ended, reconnecting");
                    }
                    _ => {
                        warn!(?e, "WebSocket error, reconnecting");
                    }
                }
                if do_reconnect(
                    &client,
                    &config,
                    &active_topics,
                    &mut event_tx,
                    &mut ws,
                    &mut shutdown_rx,
                    &mut state,
                )
                .await
                {
                    return;
                }
                last_msg = Instant::now();
            }
            Branch::Idle => {
                let elapsed = last_msg.elapsed();
                warn!(
                    ?elapsed,
                    "no server messages within idle timeout, forcing reconnect"
                );
                if do_reconnect(
                    &client,
                    &config,
                    &active_topics,
                    &mut event_tx,
                    &mut ws,
                    &mut shutdown_rx,
                    &mut state,
                )
                .await
                {
                    return;
                }
                last_msg = Instant::now();
            }
            Branch::Cmd(Some(WsCommand::Subscribe(params, id))) => {
                // Skip already-subscribed topics: the server may reject duplicates, and the set
                // drives replay.
                let new_params: Vec<String> = params
                    .into_iter()
                    .filter(|p| active_topics.insert(p.clone()))
                    .collect();
                if new_params.is_empty() {
                    debug!("subscribe: all topics already active, skipping wire send");
                } else if let Err(e) = ws
                    .send(ClientMessage::Subscribe {
                        id,
                        params: new_params,
                    })
                    .await
                {
                    debug!(?e, "subscribe send failed, will replay after reconnect");
                }
            }
            Branch::Cmd(Some(WsCommand::Unsubscribe(params, id))) => {
                // Dedup: only send for topics we're actually subscribed to.
                let to_send: Vec<String> = params
                    .into_iter()
                    .filter(|p| active_topics.remove(p))
                    .collect();
                if to_send.is_empty() {
                    debug!("unsubscribe: no matching active topics, skipping wire send");
                } else if let Err(e) = ws
                    .send(ClientMessage::Unsubscribe {
                        id,
                        params: to_send,
                    })
                    .await
                {
                    debug!(?e, "unsubscribe send failed");
                }
            }
            Branch::Cmd(Some(WsCommand::Send(msg))) => {
                if let Err(e) = ws.send(msg.clone()).await {
                    warn!(?e, "failed to send order message, reconnecting");
                    if do_reconnect(
                        &client,
                        &config,
                        &active_topics,
                        &mut event_tx,
                        &mut ws,
                        &mut shutdown_rx,
                        &mut state,
                    )
                    .await
                    {
                        return;
                    }
                    // Retry once; if it fails again, the next iteration reconnects.
                    if let Err(e) = ws.send(msg).await {
                        warn!(?e, "retry after reconnect also failed");
                    }
                    last_msg = Instant::now();
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
async fn do_reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mut mpsc::Sender<WsEvent>,
    ws: &mut WebsocketHandle,
    shutdown_rx: &mut oneshot::Receiver<()>,
    state: &mut ReconnectState,
) -> bool {
    match event_tx.try_send(WsEvent::Reconnecting) {
        Ok(()) => {}
        Err(e) if e.is_full() => {}
        Err(_) => return true,
    }

    // Reset backoff only after a stable connection (see `ReconnectState`).
    if let Some(t) = state.connected_since
        && t.elapsed() >= config.backoff_reset_after
    {
        debug!(
            uptime = ?t.elapsed(),
            "previous connection was stable; resetting backoff"
        );
        state.backoff = config.initial_backoff.max(MIN_BACKOFF);
    }
    state.connected_since = None;

    match reconnect(client, config, active_topics, event_tx, shutdown_rx, state).await {
        Ok(new_ws) => {
            *ws = new_ws;
            state.connected_since = Some(Instant::now());
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
/// Watches `shutdown_rx` during sleeps and between attempts. `state.backoff` is mutated in place.
async fn reconnect(
    client: &ManagedWsClient,
    config: &ManagedWsConfig,
    active_topics: &HashSet<String>,
    event_tx: &mpsc::Sender<WsEvent>,
    shutdown_rx: &mut oneshot::Receiver<()>,
    state: &mut ReconnectState,
) -> Result<WebsocketHandle, ReconnectError> {
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
        let jitter_ms = rand::random::<u64>() % (state.backoff.as_millis() as u64 / 2 + 1);
        let delay = state.backoff + Duration::from_millis(jitter_ms);

        info!(attempt = attempts, delay = ?delay, backoff = ?state.backoff, "attempting reconnect");

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
                        if matches!(&e, WSErrors::WsStreamEnded | WSErrors::WsClosed { .. }) {
                            warn!(?e, "replay send lost connection, retrying");
                            state.backoff = (state.backoff * 2).min(max_backoff);
                            continue;
                        }
                        return Err(ReconnectError::ReplayFailed(e));
                    }
                }
                return Ok(ws);
            }
            Err(e) => {
                warn!(?e, attempt = attempts, "reconnect failed");
                state.backoff = (state.backoff * 2).min(max_backoff);
            }
        }
    }
}

/// Returns `true` if shutdown has been signaled (sender sent, dropped, or the
/// receiver is otherwise resolved).
fn shutdown_observed(rx: &mut oneshot::Receiver<()>) -> bool {
    !matches!(rx.try_recv(), Ok(None))
}
