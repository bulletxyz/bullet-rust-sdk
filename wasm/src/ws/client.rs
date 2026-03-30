use bullet_rust_sdk::{
    types::{ClientMessage, RequestId},
    ws::client::{WebsocketConfig, WebsocketHandle},
};
use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_time::Duration;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;

/// Handle to an active WebSocket connection.
#[wasm_bindgen(js_name = WebsocketHandle)]
pub struct WasmWebsocketHandle {
    pub(crate) inner: WebsocketHandle,
}

#[wasm_bindgen(js_class = WebsocketHandle)]
impl WasmWebsocketHandle {
    /// Receive the next server message as a JSON string.
    pub async fn recv(&mut self) -> WasmResult<String> {
        let msg = self.inner.recv().await?;
        Ok(serde_json::to_string(&msg)?)
    }

    /// Subscribe to topics. `topics` is a JS `Array` of topic strings
    /// (e.g. from `WasmTopic.toString()`). `id` is an optional request ID.
    pub async fn subscribe(&mut self, topics: Array, id: Option<u64>) -> WasmResult<()> {
        let params: Vec<String> = topics.iter().filter_map(|v| v.as_string()).collect();
        Ok(self
            .inner
            .send(ClientMessage::Subscribe {
                id: id.map(RequestId::new),
                params,
            })
            .await?)
    }

    /// Unsubscribe from topics.
    pub async fn unsubscribe(&mut self, topics: Array, id: Option<u64>) -> WasmResult<()> {
        let params: Vec<String> = topics.iter().filter_map(|v| v.as_string()).collect();
        Ok(self
            .inner
            .send(ClientMessage::Unsubscribe {
                id: id.map(RequestId::new),
                params,
            })
            .await?)
    }

    /// Request the list of active subscriptions.
    #[wasm_bindgen(js_name = listSubscriptions)]
    pub async fn list_subscriptions(&mut self, id: Option<u64>) -> WasmResult<()> {
        Ok(self
            .inner
            .list_subscriptions(id.map(RequestId::new))
            .await?)
    }

    /// Place an order. `tx` is a base64-encoded signed transaction.
    #[wasm_bindgen(js_name = orderPlace)]
    pub async fn order_place(&mut self, tx: &str, id: Option<u64>) -> WasmResult<()> {
        Ok(self.inner.order_place(tx, id.map(RequestId::new)).await?)
    }

    /// Cancel an order. `tx` is a base64-encoded signed transaction.
    #[wasm_bindgen(js_name = orderCancel)]
    pub async fn order_cancel(&mut self, tx: &str, id: Option<u64>) -> WasmResult<()> {
        Ok(self.inner.order_cancel(tx, id.map(RequestId::new)).await?)
    }
}

#[wasm_bindgen(js_name = WebsocketConfig)]
pub struct WasmWebsocketConfig {
    inner: WebsocketConfig,
}

#[wasm_bindgen(js_class = WebsocketConfig)]
impl WasmWebsocketConfig {
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
