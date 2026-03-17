//! Fluent transaction builder for WASM.
//!
//! Provides a chainable builder pattern for constructing and submitting transactions.
//!
//! # JavaScript Example
//!
//! ```js
//! const response = await TransactionBuilder.new()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .signer(keypair)
//!     .send(client);
//! ```

use bullet_rust_sdk::TransactionBuilder as RustTransactionBuilder;
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;
use crate::transactions::{WasmCallMessage, WasmTransaction};

/// Fluent builder for constructing and submitting transactions.
///
/// # Example
///
/// ```js
/// // Build and send in one chain
/// const response = await TransactionBuilder.new()
///     .callMessage(callMsg)
///     .maxFee(10_000_000n)
///     .signer(keypair)
///     .send(client);
///
/// // Or build without sending
/// const tx = TransactionBuilder.new()
///     .callMessage(callMsg)
///     .maxFee(10_000_000n)
///     .signer(keypair)
///     .build(client);
///
/// // Then send later
/// const response = await client.sendTransaction(tx);
/// ```
#[wasm_bindgen(js_name = TransactionBuilder)]
pub struct WasmTransactionBuilder {
    call_message: Option<WasmCallMessage>,
    max_fee: Option<u64>,
    priority_fee_bips: u64,
    signer: Option<WasmKeypair>,
}

#[wasm_bindgen(js_class = TransactionBuilder)]
impl WasmTransactionBuilder {
    /// Create a new transaction builder.
    #[wasm_bindgen(js_name = new)]
    pub fn new() -> WasmTransactionBuilder {
        WasmTransactionBuilder {
            call_message: None,
            max_fee: None,
            priority_fee_bips: 0,
            signer: None,
        }
    }

    /// Set the call message for this transaction.
    ///
    /// This is the action to be executed (e.g., place order, withdraw, etc.).
    #[wasm_bindgen(js_name = callMessage)]
    pub fn call_message(mut self, msg: WasmCallMessage) -> WasmTransactionBuilder {
        self.call_message = Some(msg);
        self
    }

    /// Set the maximum fee (in base units) willing to pay for this transaction.
    #[wasm_bindgen(js_name = maxFee)]
    pub fn max_fee(mut self, fee: u64) -> WasmTransactionBuilder {
        self.max_fee = Some(fee);
        self
    }

    /// Set the priority fee in basis points (optional, defaults to 0).
    ///
    /// Higher priority fees may result in faster transaction processing.
    #[wasm_bindgen(js_name = priorityFeeBips)]
    pub fn priority_fee_bips(mut self, bips: u64) -> WasmTransactionBuilder {
        self.priority_fee_bips = bips;
        self
    }

    /// Set the keypair used to sign this transaction.
    pub fn signer(mut self, keypair: WasmKeypair) -> WasmTransactionBuilder {
        self.signer = Some(keypair);
        self
    }

    /// Build the signed transaction without sending it.
    ///
    /// Use this if you want to inspect the transaction or send it later
    /// via `client.sendTransaction(tx)`.
    pub fn build(self, client: &WasmTradingApi) -> WasmResult<WasmTransaction> {
        let call_message = self
            .call_message
            .ok_or_else(|| "call_message is required")?;
        let max_fee = self.max_fee.ok_or_else(|| "max_fee is required")?;
        let signer = self.signer.ok_or_else(|| "signer is required")?;

        // Use the Rust TransactionBuilder internally
        let signed = RustTransactionBuilder::new()
            .call_message(call_message.inner)
            .max_fee(u128::from(max_fee))
            .priority_fee_bips(self.priority_fee_bips)
            .signer(&signer.inner)
            .build(&client.inner)?;

        Ok(WasmTransaction { inner: signed })
    }

    /// Sign and submit the transaction to the network.
    ///
    /// Returns a JSON string of the `SubmitTxResponse`.
    pub async fn send(self, client: &WasmTradingApi) -> WasmResult<String> {
        let tx = self.build(client)?;
        client.submit_transaction(&tx).await
    }
}

impl Default for WasmTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
