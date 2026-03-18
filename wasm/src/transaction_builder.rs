//! Fluent transaction builder for WASM.
//!
//! Provides a chainable builder pattern for constructing and submitting transactions.
//!
//! # JavaScript Example
//!
//! ```js
//! // With explicit values
//! const response = await Transaction.builder()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .signer(keypair)
//!     .send(client);
//!
//! // Using client defaults (if keypair/maxFee set on client)
//! const response = await Transaction.builder()
//!     .callMessage(callMsg)
//!     .send(client);
//! ```

use bullet_exchange_interface::transaction::Gas;
use bullet_rust_sdk::Transaction as RustTransaction;
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::keypair::WasmKeypair;
use crate::transactions::{WasmCallMessage, WasmTransaction};

/// Transaction builder entry point.
///
/// Use `Transaction.builder()` to create a new builder, then chain
/// the required fields and call `.build(client)` or `.send(client)`.
///
/// # Example
///
/// ```js
/// // Build and send with explicit values
/// const response = await Transaction.builder()
///     .callMessage(callMsg)
///     .maxFee(10_000_000n)
///     .signer(keypair)
///     .send(client);
///
/// // Using client defaults
/// const response = await Transaction.builder()
///     .callMessage(callMsg)
///     .send(client);
///
/// // Or just build
/// const tx = Transaction.builder()
///     .callMessage(callMsg)
///     .build(client);
///
/// // Send later
/// const response = await client.sendTransaction(tx);
/// ```
#[wasm_bindgen(js_name = Transaction)]
pub struct WasmTransactionEntry;

#[wasm_bindgen(js_class = Transaction)]
impl WasmTransactionEntry {
    /// Create a new transaction builder.
    pub fn builder() -> WasmTransactionBuilder {
        WasmTransactionBuilder::new()
    }
}

/// Fluent builder for constructing and submitting transactions.
///
/// Created via `Transaction.builder()`.
///
/// # Required Fields
///
/// - `callMessage` - The action to execute (e.g., place order, withdraw)
///
/// # Optional Fields (fall back to client defaults if not set)
///
/// - `maxFee` - Maximum fee willing to pay (in base units)
/// - `priorityFeeBips` - Priority fee in basis points
/// - `gasLimit` - Optional gas limit [ref_time, proof_size]
/// - `signer` - Keypair to sign the transaction
#[wasm_bindgen(js_name = TransactionBuilder)]
pub struct WasmTransactionBuilder {
    call_message: Option<WasmCallMessage>,
    max_fee: Option<u64>,
    priority_fee_bips: Option<u64>,
    gas_limit: Option<[u64; 2]>,
    signer: Option<WasmKeypair>,
}

impl WasmTransactionBuilder {
    fn new() -> Self {
        WasmTransactionBuilder {
            call_message: None,
            max_fee: None,
            priority_fee_bips: None,
            gas_limit: None,
            signer: None,
        }
    }
}

#[wasm_bindgen(js_class = TransactionBuilder)]
impl WasmTransactionBuilder {
    /// Set the call message for this transaction (required).
    ///
    /// This is the action to be executed (e.g., place order, withdraw, etc.).
    #[wasm_bindgen(js_name = callMessage)]
    pub fn call_message(mut self, msg: WasmCallMessage) -> WasmTransactionBuilder {
        self.call_message = Some(msg);
        self
    }

    /// Set the maximum fee (in base units) willing to pay for this transaction.
    ///
    /// Falls back to client default if not set.
    #[wasm_bindgen(js_name = maxFee)]
    pub fn max_fee(mut self, fee: u64) -> WasmTransactionBuilder {
        self.max_fee = Some(fee);
        self
    }

    /// Set the priority fee in basis points.
    ///
    /// Higher priority fees may result in faster transaction processing.
    /// Falls back to client default if not set.
    #[wasm_bindgen(js_name = priorityFeeBips)]
    pub fn priority_fee_bips(mut self, bips: u64) -> WasmTransactionBuilder {
        self.priority_fee_bips = Some(bips);
        self
    }

    /// Set the gas limit for this transaction.
    ///
    /// Takes [ref_time, proof_size] as parameters.
    /// Falls back to client default if not set.
    #[wasm_bindgen(js_name = gasLimit)]
    pub fn gas_limit(mut self, ref_time: u64, proof_size: u64) -> WasmTransactionBuilder {
        self.gas_limit = Some([ref_time, proof_size]);
        self
    }

    /// Set the keypair used to sign this transaction.
    ///
    /// Falls back to client default if not set.
    pub fn signer(mut self, keypair: WasmKeypair) -> WasmTransactionBuilder {
        self.signer = Some(keypair);
        self
    }

    /// Build the signed transaction without sending it.
    ///
    /// Use this if you want to inspect the transaction or send it later
    /// via `client.sendTransaction(tx)`.
    ///
    /// Falls back to client defaults for maxFee, priorityFeeBips, gasLimit, and signer
    /// if not explicitly set on the builder.
    pub fn build(self, client: &WasmTradingApi) -> WasmResult<WasmTransaction> {
        let call_message = self
            .call_message
            .ok_or_else(|| "call_message is required")?;

        // Convert options to the types expected by the Rust builder
        let max_fee = self.max_fee.map(|f| f as u128);
        let gas_limit = self.gas_limit.map(Gas);
        let signer_ref = self.signer.as_ref().map(|s| &s.inner);

        // Build using the Rust Transaction builder which handles client defaults
        let signed = RustTransaction::builder()
            .call_message(call_message.inner)
            .maybe_max_fee(max_fee)
            .maybe_priority_fee_bips(self.priority_fee_bips)
            .maybe_gas_limit(gas_limit)
            .maybe_signer(signer_ref)
            .build(&client.inner)?;

        Ok(WasmTransaction { inner: signed })
    }

    /// Sign and submit the transaction to the network.
    ///
    /// Returns a JSON string of the `SubmitTxResponse`.
    ///
    /// Falls back to client defaults for maxFee, priorityFeeBips, gasLimit, and signer
    /// if not explicitly set on the builder.
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
