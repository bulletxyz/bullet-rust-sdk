//! Transaction types, builder, and submission for WASM.
//!
//! All transaction construction goes through the builder pattern:
//!
//! ```js
//! // Build and send with explicit signer
//! const response = await Transaction.builder()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .signer(keypair)
//!     .send(client);
//!
//! // External signing
//! const unsigned = Transaction.builder()
//!     .callMessage(callMsg)
//!     .maxFee(10_000_000n)
//!     .buildUnsigned(client);
//!
//! const signable = unsigned.toBytes();
//! const signature = myExternalSigner(signable);
//! const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
//!
//! // Submit later
//! await client.sendTransaction(signed);
//! ```

use std::str::FromStr;

use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::decimals::PositiveDecimal;
use bullet_exchange_interface::message::*;
use bullet_exchange_interface::time::UnixTimestampMicros;
use bullet_exchange_interface::transaction::{Gas, Transaction};
use bullet_exchange_interface::types::{
    AdminType, AssetId, ClientOrderId, FeeTier, MarketId, OrderId, OrderType, Side,
    SpotCollateralTransferDirection, TokenId, TradingMode, TriggerDirection, TriggerOrderId,
    TriggerPriceCondition, TwapId,
};
use bullet_rust_sdk::types::CallMessage;
use bullet_rust_sdk::{
    SolanaLedgerTransaction as RustSolanaLedgerTransaction,
    SolanaOffchainTransaction as RustSolanaOffchainTransaction, Transaction as RustTransaction,
    UnsignedTransaction,
};
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

// ── WasmUnsignedTransaction ───────────────────────────────────────────────────

/// An unsigned transaction ready for external signing.
///
/// Created via `Transaction.builder().buildUnsigned(client)`. The chain hash
/// is already baked in, so `toBytes()` produces signable bytes directly.
///
/// ```js
/// const signable = unsigned.toBytes();
/// const display = unsigned.toDisplayMessage();
/// const signature = myExternalSigner(signable);
/// const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
/// ```
#[wasm_bindgen(js_name = UnsignedTransaction)]
pub struct WasmUnsignedTransaction {
    pub(crate) inner: UnsignedTransaction,
}

#[wasm_bindgen(js_class = UnsignedTransaction)]
impl WasmUnsignedTransaction {
    /// Serialize into the bytes that must be signed.
    ///
    /// Borsh-serializes the transaction and appends the chain hash (32 bytes)
    /// as domain separator. Pass the resulting `Uint8Array` to your signing function.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_bytes()?)
    }

    /// Render the unsigned transaction payload as a human-readable message.
    ///
    /// Use this string in your own confirmation UI when an external wallet shows
    /// the raw Borsh bytes during `signMessage`. The wallet must still sign the
    /// bytes from `toBytes()`.
    ///
    /// @returns {string} Human-readable transaction payload for display.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// console.log(unsigned.toDisplayMessage());
    /// const signature = await wallet.signMessage(unsigned.toBytes());
    /// ```
    #[wasm_bindgen(js_name = toDisplayMessage)]
    pub fn to_display_message(&self) -> WasmResult<String> {
        Ok(self.inner.to_display_message()?)
    }

    /// Serialize into readable JSON bytes for offchain signing.
    ///
    /// External Solana wallets should sign these bytes when the backend uses
    /// the `solanaSimple` authenticator. The JSON includes `chain_name` and
    /// `chain_id`; the current sequencer offchain authenticator also requires
    /// the envelope assembled by `SolanaOffchainTransaction.fromParts(...)`.
    /// Submit the result with `client.sendOffChainTransaction(...)`.
    ///
    /// @returns {Uint8Array} UTF-8 JSON bytes to pass to `wallet.signMessage`.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// const message = unsigned.toMessageBytes();
    /// const signature = await wallet.signMessage(message);
    /// const pubKey = wallet.publicKey.toBytes();
    /// const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);
    /// await client.sendOffChainTransaction(tx);
    /// ```
    #[wasm_bindgen(js_name = toMessageBytes)]
    pub fn to_message_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_message_bytes()?)
    }

    /// Build the bytes a Ledger hardware wallet must sign.
    ///
    /// Returns the 85-byte Solana off-chain preamble (using the chain hash as
    /// application domain) concatenated with the JSON message bytes. Pass the
    /// result to `wallet.signMessage`; then assemble and submit with
    /// `SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature)` and
    /// `client.sendLedgerTransaction(tx)`.
    ///
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @returns {Uint8Array} Bytes to pass to `wallet.signMessage`.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder().callMessage(msg).buildUnsigned(client);
    /// const pubKey = ledgerWallet.publicKey.toBytes();
    /// const signableBytes = unsigned.toLedgerSignableBytes(pubKey);
    /// const signature = await ledgerWallet.signMessage(signableBytes);
    /// const tx = SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature);
    /// await client.sendLedgerTransaction(tx);
    /// ```
    #[wasm_bindgen(js_name = toLedgerSignableBytes)]
    pub fn to_ledger_signable_bytes(&self, pub_key: &[u8]) -> WasmResult<Vec<u8>> {
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(self.inner.to_ledger_signable_bytes(&pub_key)?)
    }
}

// ── WasmSolanaLedgerTransaction ──────────────────────────────────────────────

/// A Solana offchain transaction using the spec-compliant Ledger wire format.
///
/// Use this for Ledger hardware wallets. Obtain bytes to sign from
/// `unsigned.toLedgerSignableBytes(pubKey)`, sign with the Ledger, then
/// assemble with `fromParts` and submit via `client.sendLedgerTransaction`.
#[wasm_bindgen(js_name = SolanaLedgerTransaction)]
pub struct WasmSolanaLedgerTransaction {
    pub(crate) inner: RustSolanaLedgerTransaction,
}

#[wasm_bindgen(js_class = SolanaLedgerTransaction)]
impl WasmSolanaLedgerTransaction {
    /// Assemble a Ledger transaction from an unsigned transaction, a 32-byte
    /// public key, and a 64-byte Ed25519 signature.
    ///
    /// @param {UnsignedTransaction} unsignedTx - The unsigned transaction.
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @param {Uint8Array} signature - 64-byte Ed25519 signature.
    /// @returns {SolanaLedgerTransaction}
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        pub_key: &[u8],
        signature: &[u8],
    ) -> WasmResult<WasmSolanaLedgerTransaction> {
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        Ok(WasmSolanaLedgerTransaction {
            inner: RustSolanaLedgerTransaction::from_parts(unsigned_tx.inner, pub_key, signature)?,
        })
    }

    /// Serialize to the raw binary wire format.
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Serialize to wire format and base64-encode.
    /// @returns {string}
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> String {
        self.inner.to_base64()
    }
}

// ── WasmSolanaOffchainTransaction ────────────────────────────────────────────

/// A Solana offchain transaction ready for submission.
///
/// Use this with external Solana wallets when you want the wallet to sign
/// readable JSON instead of Borsh bytes.
#[wasm_bindgen(js_name = SolanaOffchainTransaction)]
pub struct WasmSolanaOffchainTransaction {
    pub(crate) inner: RustSolanaOffchainTransaction,
}

#[wasm_bindgen(js_class = SolanaOffchainTransaction)]
impl WasmSolanaOffchainTransaction {
    /// Assemble a Solana offchain transaction from an unsigned transaction, a
    /// 64-byte Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing `unsigned.toMessageBytes()`.
    ///
    /// @param {UnsignedTransaction} unsignedTx - The unsigned transaction.
    /// @param {Uint8Array} signature - 64-byte Ed25519 signature.
    /// @param {Uint8Array} pubKey - 32-byte Solana public key.
    /// @returns {SolanaOffchainTransaction}
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        signature: &[u8],
        pub_key: &[u8],
    ) -> WasmResult<WasmSolanaOffchainTransaction> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(WasmSolanaOffchainTransaction {
            inner: RustSolanaOffchainTransaction::from_parts(
                unsigned_tx.inner,
                signature,
                pub_key,
            )?,
        })
    }

    /// Borsh-serialize the Solana offchain transaction to bytes.
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_bytes()?)
    }

    /// Borsh-serialize and base64-encode the Solana offchain transaction.
    /// @returns {string}
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        Ok(self.inner.to_base64()?)
    }
}

// ── WasmTransaction (SignedTransaction) ──────────────────────────────────────

/// A signed transaction ready for submission.
///
/// Passed directly to `Client.submitTransaction` or serialised to base64 via
/// `toBase64()` for WebSocket submission.
#[wasm_bindgen(js_name = SignedTransaction)]
pub struct WasmTransaction {
    pub(crate) inner: Transaction,
}

#[wasm_bindgen(js_class = SignedTransaction)]
impl WasmTransaction {
    /// Assemble a signed transaction from an unsigned transaction, a 64-byte
    /// Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing the bytes from `unsigned.toBytes()`.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        unsigned_tx: WasmUnsignedTransaction,
        signature: &[u8],
        pub_key: &[u8],
    ) -> WasmResult<WasmTransaction> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| format!("expected 64-byte signature, got {}", signature.len()))?;
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| format!("expected 32-byte public key, got {}", pub_key.len()))?;
        Ok(WasmTransaction {
            inner: RustTransaction::from_parts(unsigned_tx.inner, signature, pub_key),
        })
    }

    /// Borsh-serialize the signed transaction to bytes.
    ///
    /// Useful for comparing two signed transactions byte-by-byte.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(RustTransaction::to_bytes(&self.inner)?)
    }

    /// Borsh-serialise and base64-encode the transaction.
    ///
    /// Use this when you need to pass the transaction over a WebSocket
    /// connection (e.g. `WebsocketHandle.orderPlace`).
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        Ok(RustTransaction::to_base64(&self.inner)?)
    }
}

// ── Transaction builder ──────────────────────────────────────────────────────

/// Transaction builder entry point.
///
/// Use `Transaction.builder()` to create a new builder, then chain
/// the required fields and call `.build(client)`, `.buildUnsigned(client)`,
/// or `.send(client)`.
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
/// - `generation` - Uniqueness generation value (default: monotonic unix timestamp in microseconds)
/// - `signer` - Keypair to sign the transaction (not required for `buildUnsigned`)
#[wasm_bindgen(js_name = TransactionBuilder)]
pub struct WasmTransactionBuilder {
    call_message: Option<WasmCallMessage>,
    max_fee: Option<u64>,
    priority_fee_bips: Option<u64>,
    gas_limit: Option<[u64; 2]>,
    generation: Option<u64>,
    signer: Option<WasmKeypair>,
}

impl WasmTransactionBuilder {
    fn new() -> Self {
        WasmTransactionBuilder {
            call_message: None,
            max_fee: None,
            priority_fee_bips: None,
            gas_limit: None,
            generation: None,
            signer: None,
        }
    }
}

#[wasm_bindgen(js_class = TransactionBuilder)]
impl WasmTransactionBuilder {
    /// Set the call message for this transaction (required).
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

    /// Set the priority fee in basis points.
    #[wasm_bindgen(js_name = priorityFeeBips)]
    pub fn priority_fee_bips(mut self, bips: u64) -> WasmTransactionBuilder {
        self.priority_fee_bips = Some(bips);
        self
    }

    /// Set the gas limit for this transaction.
    ///
    /// Takes [ref_time, proof_size] as parameters.
    #[wasm_bindgen(js_name = gasLimit)]
    pub fn gas_limit(mut self, ref_time: u64, proof_size: u64) -> WasmTransactionBuilder {
        self.gas_limit = Some([ref_time, proof_size]);
        self
    }

    /// Override the uniqueness generation value.
    ///
    /// Defaults to a monotonic unix timestamp in microseconds, giving a
    /// ~5ms deduplication window with the sequencer's 5000-generation window.
    /// @param {bigint} generation - The generation value to use.
    /// @returns {TransactionBuilder}
    pub fn generation(mut self, generation: u64) -> WasmTransactionBuilder {
        self.generation = Some(generation);
        self
    }

    /// Set the keypair used to sign this transaction.
    pub fn signer(mut self, keypair: WasmKeypair) -> WasmTransactionBuilder {
        self.signer = Some(keypair);
        self
    }

    /// Build the unsigned transaction without signing.
    ///
    /// Returns an `UnsignedTransaction` that can be signed externally:
    ///
    /// ```js
    /// const unsigned = Transaction.builder()
    ///     .callMessage(callMsg)
    ///     .maxFee(10_000_000n)
    ///     .buildUnsigned(client);
    ///
    /// const signable = unsigned.toBytes();
    /// const signature = myExternalSigner(signable);
    /// const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
    /// ```
    #[wasm_bindgen(js_name = buildUnsigned)]
    pub fn build_unsigned(self, client: &WasmTradingApi) -> WasmResult<WasmUnsignedTransaction> {
        let call_message = self.call_message.ok_or("call_message is required")?;

        let max_fee = self.max_fee.map(|f| f as u128).unwrap_or_else(|| client.inner.max_fee().0);
        let priority_fee_bips =
            self.priority_fee_bips.unwrap_or_else(|| client.inner.max_priority_fee_bips().0);
        let gas_limit = self.gas_limit.map(Gas).or_else(|| client.inner.gas_limit());

        let unsigned = UnsignedTransaction::builder()
            .call_message(call_message.inner)
            .max_fee(max_fee)
            .priority_fee_bips(priority_fee_bips)
            .maybe_gas_limit(gas_limit)
            .maybe_generation(self.generation)
            .client(&client.inner)
            .build()?;

        Ok(WasmUnsignedTransaction { inner: unsigned })
    }

    /// Build the signed transaction without sending it.
    pub fn build(self, client: &WasmTradingApi) -> WasmResult<WasmTransaction> {
        let call_message = self.call_message.ok_or("call_message is required")?;

        let max_fee = self.max_fee.map(|f| f as u128);
        let gas_limit = self.gas_limit.map(Gas);
        let signer_ref = self.signer.as_ref().map(|s| &s.inner);

        let signed = RustTransaction::builder()
            .call_message(call_message.inner)
            .maybe_max_fee(max_fee)
            .maybe_priority_fee_bips(self.priority_fee_bips)
            .maybe_gas_limit(gas_limit)
            .maybe_generation(self.generation)
            .maybe_signer(signer_ref)
            .client(&client.inner)
            .build()?;

        Ok(WasmTransaction { inner: signed })
    }

    /// Sign and submit the transaction to the network.
    /// @param {Client} client - The trading API client.
    /// @returns {Promise<SubmitTxResponse>}
    pub async fn send(
        self,
        client: &WasmTradingApi,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let tx = self.build(client)?;
        client.send_transaction(&tx).await
    }
}

impl Default for WasmTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Client convenience methods ───────────────────────────────────────────────

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Send a signed transaction to the network via REST.
    /// @param {Transaction} tx - A signed transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendTransaction)]
    pub async fn send_transaction(
        &self,
        tx: &WasmTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Send a Solana offchain transaction to the sequencer via REST.
    /// @param {SolanaOffchainTransaction} tx - A signed Solana offchain transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendOffChainTransaction)]
    pub async fn send_off_chain_transaction(
        &self,
        tx: &WasmSolanaOffchainTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_offchain_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Send a Ledger-signed transaction to the sequencer via REST.
    ///
    /// Use this with Ledger hardware wallets. Sign with
    /// `unsigned.toLedgerSignableBytes(pubKey)`, then assemble via
    /// `SolanaLedgerTransaction.fromParts(unsigned, pubKey, signature)`.
    ///
    /// @param {SolanaLedgerTransaction} tx - A signed Ledger transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendLedgerTransaction)]
    pub async fn send_ledger_transaction(
        &self,
        tx: &WasmSolanaLedgerTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self.inner.send_ledger_transaction(&tx.inner).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }

    /// Sign and submit a call message in one step.
    ///
    /// This is a convenience method that wraps
    /// `Transaction.builder().callMessage(msg).send(client)` into a single call using the
    /// client's default keypair, max fee, and gas settings.
    ///
    /// @param {CallMessage} msg - A call message (e.g. from `User.placeOrders(...)`)
    /// @returns {Promise<SubmitTxResponse>}
    ///
    /// @example
    /// ```js
    /// const order = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false);
    /// const resp = await client.sendCallMessage(User.placeOrders(0, [order], false));
    /// ```
    #[wasm_bindgen(js_name = sendCallMessage)]
    pub async fn send_call_message(
        &self,
        msg: WasmCallMessage,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let signed =
            RustTransaction::builder().call_message(msg.inner).client(&self.inner).build()?;
        let resp = self.inner.send_transaction(&signed).await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }
}
