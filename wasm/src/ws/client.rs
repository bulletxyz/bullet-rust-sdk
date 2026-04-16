use bullet_rust_sdk::{
    types::{ClientMessage, RequestId},
    ws::{
        client::{WebsocketConfig, WebsocketHandle},
        models::ServerMessage,
    },
};
use js_sys::{Array, Function};
use wasm_bindgen::prelude::*;
use web_time::Duration;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;

#[wasm_bindgen]
extern "C" {
    /// Typed array parameter for subscribe/unsubscribe.
    #[wasm_bindgen(typescript_type = "Array<Topic>")]
    pub type TopicArray;
}

/// A message received from the WebSocket server.
///
/// @example
/// ```js
/// const msg = await ws.recv();
/// if (msg.type === "depthUpdate") {
///   console.log(msg.data);
/// }
/// ```
#[wasm_bindgen(js_name = ServerMessage)]
pub struct WasmServerMessage {
    inner: ServerMessage,
}

#[wasm_bindgen(js_class = ServerMessage)]
impl WasmServerMessage {
    /// The message type discriminant, e.g. `"depthUpdate"`, `"aggTrade"`,
    /// `"status"`, `"error"`, `"unknown"`, etc.
    /// @returns {string}
    #[wasm_bindgen(getter, js_name = type)]
    pub fn msg_type(&self) -> String {
        match &self.inner {
            ServerMessage::Tagged(t) => t.as_ref().to_string(),
            other => other.as_ref().to_string(),
        }
    }

    /// The message payload as a plain JS object.
    /// @returns {object}
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner).unwrap_or(JsValue::NULL)
    }

    /// `true` if this is an error message (tagged or untagged).
    /// @returns {boolean}
    #[wasm_bindgen(js_name = isError)]
    pub fn is_error(&self) -> bool {
        self.inner.is_error()
    }

    /// The request ID, if the message carried one.
    /// @returns {number | undefined}
    #[wasm_bindgen(js_name = requestId, getter)]
    pub fn request_id(&self) -> Option<u64> {
        self.inner.request_id().map(|id| id.as_u64())
    }
}

/// Handle to an active WebSocket connection.
#[wasm_bindgen(js_name = WebsocketHandle)]
pub struct WasmWebsocketHandle {
    pub(crate) inner: WebsocketHandle,
}

/// Resolve a JS value to a topic string.
/// Accepts either a plain string or a `Topic` object (calls `toString()`).
fn resolve_topic(v: JsValue) -> Option<String> {
    v.as_string().or_else(|| {
        let to_str = js_sys::Reflect::get(&v, &"toString".into()).ok()?;
        let func = to_str.dyn_ref::<Function>()?;
        func.call0(&v).ok()?.as_string()
    })
}

#[wasm_bindgen(js_class = WebsocketHandle)]
impl WasmWebsocketHandle {
    /// Receive the next server message.
    /// @returns {Promise<ServerMessage>}
    pub async fn recv(&mut self) -> WasmResult<WasmServerMessage> {
        let msg = self.inner.recv().await?;
        Ok(WasmServerMessage { inner: msg })
    }

    /// Subscribe to topics.
    /// @param {Array<Topic>} topics - Array of `Topic` objects (e.g. `[Topic.aggTrade("BTC-USD")]`).
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    pub async fn subscribe(&mut self, topics: TopicArray, id: Option<u64>) -> WasmResult<()> {
        let arr: &Array = topics.unchecked_ref();
        let params: Vec<String> = arr.iter().filter_map(resolve_topic).collect();
        Ok(self
            .inner
            .send(ClientMessage::Subscribe {
                id: id.map(RequestId::new),
                params,
            })
            .await?)
    }

    /// Unsubscribe from topics.
    /// @param {Array<Topic>} topics - Array of `Topic` objects.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    pub async fn unsubscribe(&mut self, topics: TopicArray, id: Option<u64>) -> WasmResult<()> {
        let arr: &Array = topics.unchecked_ref();
        let params: Vec<String> = arr.iter().filter_map(resolve_topic).collect();
        Ok(self
            .inner
            .send(ClientMessage::Unsubscribe {
                id: id.map(RequestId::new),
                params,
            })
            .await?)
    }

    /// Request the list of active subscriptions.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = listSubscriptions)]
    pub async fn list_subscriptions(&mut self, id: Option<u64>) -> WasmResult<()> {
        Ok(self
            .inner
            .list_subscriptions(id.map(RequestId::new))
            .await?)
    }

    /// Place an order.
    /// @param {string} tx - Base64-encoded signed transaction.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = orderPlace)]
    pub async fn order_place(&mut self, tx: &str, id: Option<u64>) -> WasmResult<()> {
        Ok(self.inner.order_place(tx, id.map(RequestId::new)).await?)
    }

    /// Cancel an order.
    /// @param {string} tx - Base64-encoded signed transaction.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = orderCancel)]
    pub async fn order_cancel(&mut self, tx: &str, id: Option<u64>) -> WasmResult<()> {
        Ok(self.inner.order_cancel(tx, id.map(RequestId::new)).await?)
    }

    /// Place an order using a signed transaction object.
    ///
    /// Convenience wrapper that handles base64 encoding internally.
    /// @param {Transaction} tx - A signed transaction.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = placeOrder)]
    pub async fn place_order(
        &mut self,
        tx: &crate::transaction_builder::WasmTransaction,
        id: Option<u64>,
    ) -> WasmResult<()> {
        Ok(self
            .inner
            .place_order(&tx.inner, id.map(RequestId::new))
            .await?)
    }

    /// Cancel an order using a signed transaction object.
    ///
    /// Convenience wrapper that handles base64 encoding internally.
    /// @param {Transaction} tx - A signed transaction.
    /// @param {number} [id] - Optional request ID for correlating the server response.
    /// @returns {Promise<void>}
    #[wasm_bindgen(js_name = cancelOrder)]
    pub async fn cancel_order(
        &mut self,
        tx: &crate::transaction_builder::WasmTransaction,
        id: Option<u64>,
    ) -> WasmResult<()> {
        Ok(self
            .inner
            .cancel_order(&tx.inner, id.map(RequestId::new))
            .await?)
    }
}

/// Configuration for a WebSocket connection.
#[wasm_bindgen(js_name = WebsocketConfig)]
pub struct WasmWebsocketConfig {
    inner: WebsocketConfig,
}

#[wasm_bindgen(js_class = WebsocketConfig)]
impl WasmWebsocketConfig {
    /// Create a new WebSocket configuration.
    /// @param {number} [connection_timeout] - Connection timeout in seconds.
    /// @returns {WebsocketConfig}
    #[wasm_bindgen]
    pub fn new(connection_timeout: Option<u64>) -> Self {
        Self {
            inner: WebsocketConfig::builder()
                .maybe_connection_timeout(connection_timeout.map(Duration::from_secs))
                .build(),
        }
    }
}

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Open a WebSocket connection with the default handshake timeout.
    /// @param {WebsocketConfig} [config] - Optional connection configuration.
    /// @returns {Promise<WebsocketHandle>}
    #[wasm_bindgen(js_name = connectWs)]
    pub async fn connect_ws(
        &self,
        config: Option<WasmWebsocketConfig>,
    ) -> WasmResult<WasmWebsocketHandle> {
        Ok(WasmWebsocketHandle {
            inner: self
                .inner
                .connect_ws()
                .maybe_config(config.map(|c| c.inner))
                .call()
                .await?,
        })
    }
}
