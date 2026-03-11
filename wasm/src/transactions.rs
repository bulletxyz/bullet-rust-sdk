use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use borsh::BorshDeserialize;
use bullet_rust_sdk::{Client, types::{CallMessage, Transaction}};
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;

#[wasm_bindgen(js_class = TradingApi)]
impl WasmTradingApi {
    /// Build, sign, and base64-encode a transaction in one step.
    ///
    /// - `call_msg_json` – JSON-encoded `CallMessage`
    /// - `max_fee`       – maximum fee in base units
    /// - `keypair`       – signing keypair
    ///
    /// Returns a base64-encoded borsh-serialised signed transaction, ready to
    /// pass to `submitTransaction` or a WebSocket `orderPlace` call.
    #[wasm_bindgen(js_name = buildSignedTransaction)]
    pub fn build_signed_transaction(
        &self,
        call_msg_json: &str,
        max_fee: u64,
        keypair: &WasmKeypair,
    ) -> WasmResult<String> {
        let call_msg: CallMessage = serde_json::from_str(call_msg_json)?;
        let unsigned = self.inner.build_transaction(call_msg, u128::from(max_fee))?;
        let signed = self.inner.sign_transaction(unsigned, &keypair.inner)?;
        Ok(Client::sign_to_base64(&signed)?)
    }

    /// Submit a base64-encoded signed transaction via REST.
    ///
    /// Returns a JSON string of the `SubmitTxResponse`.
    #[wasm_bindgen(js_name = submitTransaction)]
    pub async fn submit_transaction(&self, signed_tx_b64: &str) -> WasmResult<String> {
        let bytes = BASE64.decode(signed_tx_b64)?;
        let signed = Transaction::try_from_slice(&bytes)?;
        let resp = self.inner.submit_transaction(&signed).await?;
        Ok(serde_json::to_string(&resp)?)
    }
}
