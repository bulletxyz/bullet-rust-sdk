//! Raw WebSocket transport; use [`super::managed::ManagedWebsocket`] for automatic reconnects.
//! Protocol ping/pong is handled internally. Unparseable messages become `ServerMessage::Unknown`.

use std::ops::Deref;

use bon::{Builder, bon};
use futures::{FutureExt, SinkExt, StreamExt, select};
use futures_timer::Delay;
use tracing::{debug, warn};
use web_time::Duration;

use super::models::{ServerMessage, TaggedMessage};
use super::topics::Topic;
use crate::errors::WSErrors;
use crate::types::{ClientMessage, OrderParams, RequestId};
use crate::{Client, SDKResult};

const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 10;

/// Raw socket handle: `Send`, but not `Sync`.
/// Give one task ownership and communicate through channels when sharing it.
/// Requests use optional IDs to correlate server acknowledgements; sending does not wait for one.
/// `Deref` exposes the underlying `reqwest_websocket::WebSocket`.
pub struct WebsocketHandle {
    socket: reqwest_websocket::WebSocket,
}

impl WebsocketHandle {
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

/// WebSocket handshake configuration.
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

    /// Serialize and send a request; read its acknowledgement with [`Self::recv`].
    pub async fn send(&mut self, msg: ClientMessage) -> SDKResult<(), WSErrors> {
        let string_msg = serde_json::to_string(&msg)?;
        self.socket
            .send(reqwest_websocket::Message::Text(string_msg))
            .await?;
        Ok(())
    }

    /// Receive the next application message; protocol ping/pong is handled internally.
    ///
    /// Unparseable text returns [`ServerMessage::Unknown`] with the error and raw text.
    /// A close frame returns [`WSErrors::WsClosed`]; an exhausted stream returns
    /// [`WSErrors::WsStreamEnded`]. Transport errors propagate.
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

    /// Subscribe to topics; `id` correlates the server acknowledgement.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use bullet_rust_sdk::{Client, OrderbookDepth, Topic};
    /// let client = Client::mainnet().await?;
    /// let mut ws = client.connect_ws().call().await?;
    /// ws.subscribe([Topic::depth("BTC-USD", OrderbookDepth::D20)], None).await?;
    /// println!("{:?}", ws.recv().await?);
    /// # Ok(())
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

    /// Request active subscriptions; `id` correlates the server response.
    pub async fn list_subscriptions(&mut self, id: Option<RequestId>) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::ListSubscriptions { id }).await
    }

    /// Place orders with a base64-encoded signed transaction; `id` correlates the response.
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

    /// Cancel orders with a base64-encoded signed transaction; `id` correlates the response.
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

    /// Amend orders with a base64-encoded signed transaction; `id` correlates the response.
    pub async fn order_amend(
        &mut self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::OrderAmend {
            id,
            params: OrderParams { tx: tx.into() },
        })
        .await
    }

    /// Cancel all orders with a base64-encoded signed transaction; `id` correlates the response.
    pub async fn order_cancel_all(
        &mut self,
        tx: impl Into<String>,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        self.send(ClientMessage::OrderCancelAll {
            id,
            params: OrderParams { tx: tx.into() },
        })
        .await
    }

    /// Place orders, encoding the signed transaction for [`Self::order_place`].
    pub async fn place_order(
        &mut self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_place(base64, id).await
    }

    /// Cancel orders, encoding the signed transaction for [`Self::order_cancel`].
    pub async fn cancel_order(
        &mut self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel(base64, id).await
    }

    /// Amend orders, encoding the signed transaction for [`Self::order_amend`].
    pub async fn amend_order(
        &mut self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_amend(base64, id).await
    }

    /// Cancel all orders, encoding the signed transaction for [`Self::order_cancel_all`].
    pub async fn cancel_all_orders(
        &mut self,
        signed: &crate::SignedTransaction,
        id: Option<RequestId>,
    ) -> SDKResult<(), WSErrors> {
        let base64 =
            crate::Transaction::to_base64(signed).map_err(|e| WSErrors::WsError(e.to_string()))?;
        self.order_cancel_all(base64, id).await
    }
}
