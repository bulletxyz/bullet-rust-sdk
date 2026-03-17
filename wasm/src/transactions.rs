use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::*;
use bullet_exchange_interface::time::UnixTimestampMicros;
use bullet_exchange_interface::transaction::Transaction;
use bullet_exchange_interface::types::{
    AdminType, AssetId, ClientOrderId, FeeTier, MarketId, OrderId, OrderType, Side,
    SpotCollateralTransferDirection, TokenId, TradingMode, TriggerDirection,
    TriggerOrderId, TriggerPriceCondition, TwapId,
};
use bullet_rust_sdk::types::CallMessage;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a base58 address string.
fn parse_addr(s: &str) -> Result<Address, String> {
    s.parse()
}

/// Parse a decimal string into `PositiveDecimal`.
fn parse_dec(s: &str) -> Result<PositiveDecimal, String> {
    PositiveDecimal::from_str(s).map_err(|e| format!("{e:?}"))
}

/// Parse a decimal string into `SurrogateDecimal` (used by funding/interest rate fields).
fn parse_surrogate_dec(
    s: &str,
) -> Result<bullet_exchange_interface::decimals::SurrogateDecimal, String> {
    use bullet_exchange_interface::decimals::SurrogateDecimal;
    SurrogateDecimal::from_str(s).map_err(|e| format!("{e:?}"))
}

/// Parse a JSON string into a serde-deserializable type.
fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

// ── WasmCallMessage ───────────────────────────────────────────────────────────

/// An opaque call message to be included in a transaction.
///
/// Construct via the namespace modules: `User`, `Public`, `Admin`, `Keeper`, `Vault`.
/// Each module has static factory methods, e.g. `User.deposit(0, "100.0")`.
#[wasm_bindgen(js_name = CallMessage)]
pub struct WasmCallMessage {
    pub(crate) inner: CallMessage,
}

// ── Generated namespace structs (User, Public, Admin, Keeper, Vault) ─────────
//
// Each struct is a JS namespace with static factory methods that return
// `WasmCallMessage` instances. Generated from the Transaction schema by build.rs.
include!(concat!(env!("OUT_DIR"), "/call_message_factories.rs"));

// ── WasmTransaction ───────────────────────────────────────────────────────────

/// An opaque handle to a signed `Transaction`.
///
/// Passed directly to `Client.submitTransaction` or serialised to base64 via
/// `toBase64()` for WebSocket submission — no redundant encode/decode at the
/// JS boundary.
#[wasm_bindgen(js_name = Transaction)]
pub struct WasmTransaction {
    pub(crate) inner: Transaction,
}

#[wasm_bindgen(js_class = Transaction)]
impl WasmTransaction {
    /// Borsh-serialise and base64-encode the transaction.
    ///
    /// Use this when you need to pass the transaction over a WebSocket
    /// connection (e.g. `WebsocketHandle.orderPlace`).
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        use bullet_rust_sdk::Client;
        Ok(Client::sign_to_base64(&self.inner)?)
    }
}

// ── Client methods ────────────────────────────────────────────────────────────

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Build and sign a transaction, returning an opaque `Transaction` handle.
    ///
    /// - `call_msg` – a `CallMessage` constructed via a factory method
    /// - `max_fee`  – maximum fee in base units
    /// - `keypair`  – signing keypair
    #[wasm_bindgen(js_name = buildSignedTransaction)]
    pub fn build_signed_transaction(
        &self,
        call_msg: WasmCallMessage,
        max_fee: u64,
        keypair: &WasmKeypair,
    ) -> WasmResult<WasmTransaction> {
        let unsigned = self
            .inner
            .build_transaction(call_msg.inner, u128::from(max_fee))?;
        let signed = self.inner.sign_transaction(unsigned, &keypair.inner)?;
        Ok(WasmTransaction { inner: signed })
    }

    /// Submit a signed transaction via REST.
    ///
    /// Returns a JSON string of the `SubmitTxResponse`.
    #[wasm_bindgen(js_name = submitTransaction)]
    pub async fn submit_transaction(&self, tx: &WasmTransaction) -> WasmResult<String> {
        let resp = self.inner.submit_transaction(&tx.inner).await?;
        Ok(serde_json::to_string(&resp)?)
    }

    /// Alias for `submitTransaction`.
    ///
    /// Submits a pre-signed transaction to the network. This is useful when
    /// using the `TransactionBuilder` pattern:
    ///
    /// ```js
    /// const tx = TransactionBuilder.new()
    ///     .callMessage(callMsg)
    ///     .maxFee(10_000_000n)
    ///     .signer(keypair)
    ///     .build(client);
    ///
    /// const response = await client.sendTransaction(tx);
    /// ```
    #[wasm_bindgen(js_name = sendTransaction)]
    pub async fn send_transaction(&self, tx: &WasmTransaction) -> WasmResult<String> {
        self.submit_transaction(tx).await
    }
}
