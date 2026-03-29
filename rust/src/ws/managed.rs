//! Auto-reconnecting WebSocket client with subscription replay.
//!
//! `ManagedWebsocket` wraps [`WebsocketHandle`] and automatically reconnects
//! on disconnect, replaying all active subscriptions after each reconnect.
//!
//! # Example
//!
//! ```no_run
//! use bullet_rust_sdk::{Client, Topic, OrderbookDepth};
//! use bullet_rust_sdk::types::RequestId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let api = Client::mainnet().await?;
//! let mut ws = api.connect_ws_managed().await?;
//!
//! ws.subscribe([
//!     Topic::agg_trade("BTC-USD"),
//!     Topic::depth("ETH-USD", OrderbookDepth::D10),
//! ], Some(RequestId::new(1))).await?;
//!
//! // recv() reconnects automatically on disconnect
//! loop {
//!     let msg = ws.recv().await?;
//!     println!("{:?}", msg);
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashSet;

use futures_timer::Delay;
use tracing::{debug, warn};
use web_time::Duration;

use super::client::{WebsocketConfig, WebsocketHandle};
use super::models::ServerMessage;
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::types::{ClientMessage, RequestId, SignedTransaction};
use crate::{Client, SDKResult};

/// Exponential backoff base delay.
const BACKOFF_BASE_MS: u64 = 500;
/// Maximum backoff delay.
const BACKOFF_MAX_MS: u64 = 30_000;

/// An auto-reconnecting WebSocket client.
///
/// Subscriptions are tracked and replayed automatically after each reconnect.
/// Exponential backoff (500ms base, 30s max) is applied between reconnect attempts.
///
/// Obtain via [`Client::connect_ws_managed`].
pub struct ManagedWebsocket {
    ws_url: String,
    http_client: reqwest::Client,
    config: WebsocketConfig,
    subscriptions: HashSet<String>,
    handle: Option<WebsocketHandle>,
    backoff_current: Duration,
}

impl ManagedWebsocket {
    pub(crate) async fn new(client: &Client) -> SDKResult<Self> {
        let ws_url = client.ws_url().to_string();
        let http_client = client.http_client();
        let config = WebsocketConfig::default();

        let mut managed = Self {
            ws_url,
            http_client,
            config,
            subscriptions: HashSet::new(),
            handle: None,
            backoff_current: Duration::from_millis(BACKOFF_BASE_MS),
        };
        // Eagerly connect so errors surface at construction time.
        managed.ensure_connected().await?;
        Ok(managed)
    }

    /// Subscribe to one or more topics.
    ///
    /// Subscription strings are tracked and replayed on reconnect.
    pub async fn subscribe(
        &mut self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let params: Vec<String> = topics.into_iter().map(|t| t.to_string()).collect();
        for p in &params {
            self.subscriptions.insert(p.clone());
        }
        self.subscribe_raw(params, id).await
    }

    /// Unsubscribe from one or more topics.
    pub async fn unsubscribe(
        &mut self,
        topics: impl IntoIterator<Item = Topic>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let params: Vec<String> = topics.into_iter().map(|t| t.to_string()).collect();
        for p in &params {
            self.subscriptions.remove(p);
        }
        let handle = self.handle.as_mut().ok_or(WSErrors::WsStreamEnded)?;
        handle
            .send(ClientMessage::Unsubscribe { id, params })
            .await
    }

    /// Receive the next message, reconnecting automatically on disconnect.
    ///
    /// On reconnect, all active subscriptions are replayed.
    /// Errors other than `WsClosed`/`WsStreamEnded` are returned immediately.
    pub async fn recv(&mut self) -> SDKResult<ServerMessage, WSErrors> {
        loop {
            self.ensure_connected().await?;

            let result = self
                .handle
                .as_mut()
                .expect("ensure_connected guarantees a handle")
                .recv()
                .await;

            match result {
                Ok(msg) => {
                    self.backoff_current = Duration::from_millis(BACKOFF_BASE_MS);
                    return Ok(msg);
                }
                Err(WSErrors::WsClosed { code, reason }) => {
                    warn!(?code, %reason, "WebSocket closed, reconnecting");
                    self.handle = None;
                    self.sleep_backoff().await;
                }
                Err(WSErrors::WsStreamEnded) => {
                    warn!("WebSocket stream ended, reconnecting");
                    self.handle = None;
                    self.sleep_backoff().await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Place an order using a typed signed transaction.
    pub async fn order_place_signed(
        &mut self,
        tx: &SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.ensure_connected().await?;
        self.handle
            .as_mut()
            .expect("ensure_connected guarantees a handle")
            .order_place_signed(tx, id)
            .await
    }

    /// Cancel an order using a typed signed transaction.
    pub async fn order_cancel_signed(
        &mut self,
        tx: &SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.ensure_connected().await?;
        self.handle
            .as_mut()
            .expect("ensure_connected guarantees a handle")
            .order_cancel_signed(tx, id)
            .await
    }

    /// Send a raw subscribe message (used by the WASM bridge).
    pub(crate) async fn subscribe_raw(
        &mut self,
        params: Vec<String>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let handle = self.handle.as_mut().ok_or(WSErrors::WsStreamEnded)?;
        handle
            .send(ClientMessage::Subscribe { id, params })
            .await
    }

    // ── Private ───────────────────────────────────────────────────────────────

    async fn ensure_connected(&mut self) -> SDKResult<(), WSErrors> {
        if self.handle.is_some() {
            return Ok(());
        }

        debug!("ManagedWebsocket: (re)connecting to {}", self.ws_url);
        use reqwest_websocket::Upgrade;

        let response = self
            .http_client
            .clone()
            .get(&self.ws_url)
            .upgrade()
            .send()
            .await?;

        let websocket = response.into_websocket().await?;
        let mut handle = WebsocketHandle::from_socket(websocket);
        handle
            .wait_for_connected(self.config.connection_timeout)
            .await?;

        // Replay subscriptions
        if !self.subscriptions.is_empty() {
            let params: Vec<String> = self.subscriptions.iter().cloned().collect();
            debug!("ManagedWebsocket: replaying {} subscriptions", params.len());
            handle
                .send(ClientMessage::Subscribe { id: None, params })
                .await?;
        }

        self.handle = Some(handle);
        Ok(())
    }

    async fn sleep_backoff(&mut self) {
        let delay = self.backoff_current;
        debug!("ManagedWebsocket: backoff {:?}", delay);
        #[allow(clippy::useless_conversion)]
        Delay::new(delay.try_into().unwrap_or(std::time::Duration::from_millis(
            BACKOFF_BASE_MS,
        )))
        .await;
        self.backoff_current = std::cmp::min(
            self.backoff_current * 2,
            Duration::from_millis(BACKOFF_MAX_MS),
        );
    }
}
