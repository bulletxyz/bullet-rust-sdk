use bullet_rust_sdk::types::RequestId;
use bullet_rust_sdk::ws::ManagedWebsocket;
use bullet_rust_sdk::Topic;
use js_sys::Array;
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::transactions::WasmTransaction;

/// Auto-reconnecting WebSocket client with subscription replay.
///
/// Obtain via `Client.connectWsManaged()`.
#[wasm_bindgen(js_name = ManagedWebsocket)]
pub struct WasmManagedWebsocket {
    inner: ManagedWebsocket,
}

#[wasm_bindgen(js_class = ManagedWebsocket)]
impl WasmManagedWebsocket {
    /// Receive the next server message as a JSON string.
    ///
    /// Reconnects automatically on disconnect and replays subscriptions.
    pub async fn recv(&mut self) -> WasmResult<String> {
        let msg = self.inner.recv().await?;
        Ok(serde_json::to_string(&msg)?)
    }

    /// Subscribe to topics. `topics` is a JS `Array` of topic strings.
    pub async fn subscribe(&mut self, topics: Array, id: Option<u64>) -> WasmResult<()> {
        let params: Vec<String> = topics.iter().filter_map(|v| v.as_string()).collect();
        Ok(self
            .inner
            .subscribe_raw(params, id.map(RequestId::new))
            .await?)
    }

    /// Unsubscribe from topics. `topics` is a JS `Array` of topic strings.
    pub async fn unsubscribe(&mut self, topics: Array, id: Option<u64>) -> WasmResult<()> {
        let params: Vec<Topic> = topics
            .iter()
            .filter_map(|v| v.as_string())
            .filter_map(|s| parse_topic(&s))
            .collect();
        Ok(self
            .inner
            .unsubscribe(params, id.map(RequestId::new))
            .await?)
    }

    /// Place an order using a typed signed transaction.
    #[wasm_bindgen(js_name = orderPlaceSigned)]
    pub async fn order_place_signed(
        &mut self,
        tx: &WasmTransaction,
        id: Option<u64>,
    ) -> WasmResult<()> {
        Ok(self
            .inner
            .order_place_signed(&tx.inner, id.map(RequestId::new))
            .await?)
    }

    /// Cancel an order using a typed signed transaction.
    #[wasm_bindgen(js_name = orderCancelSigned)]
    pub async fn order_cancel_signed(
        &mut self,
        tx: &WasmTransaction,
        id: Option<u64>,
    ) -> WasmResult<()> {
        Ok(self
            .inner
            .order_cancel_signed(&tx.inner, id.map(RequestId::new))
            .await?)
    }
}

/// Attempt to parse a topic string into a `Topic` for unsubscribe.
/// Unknown strings are silently skipped — the server handles unknown topics gracefully.
fn parse_topic(s: &str) -> Option<Topic> {
    if let Some(sym) = s.strip_suffix("@aggTrade") {
        return Some(Topic::agg_trade(sym));
    }
    if let Some(sym) = s.strip_suffix("@bookTicker") {
        return Some(Topic::book_ticker(sym));
    }
    if s == "!ticker@arr" {
        return Some(Topic::all_tickers());
    }
    None
}

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Connect to the WebSocket API with automatic reconnection.
    #[wasm_bindgen(js_name = connectWsManaged)]
    pub async fn connect_ws_managed(&self) -> WasmResult<WasmManagedWebsocket> {
        let inner = ManagedWebsocket::new(&self.inner).await?;
        Ok(WasmManagedWebsocket { inner })
    }
}
