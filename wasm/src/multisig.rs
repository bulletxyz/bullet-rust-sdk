//! Multisig (M-of-N) transaction support for WASM.
//!
//! Mirrors `rust/src/multisig.rs`. Multisig transactions are submitted through
//! the same spec-compliant Solana offchain wire format used for Ledger
//! signing, extended to N signers.
//!
//! ```js
//! const config = new MultisigConfig(2, [pubkeyA, pubkeyB, pubkeyC]);
//!
//! const unsigned = Transaction.builder()
//!     .callMessage(callMsg)
//!     .buildUnsigned(client);
//!
//! const tx = new SolanaLedgerMultisigTransaction(unsigned, config);
//! const signature = await ledgerWallet.signMessage(tx.signableBytes());
//! tx.addSignature(pubkeyA, signature);
//! // ... collect more signatures until tx.isComplete() ...
//! await client.sendLedgerMultisigTransaction(tx);
//! ```

use bullet_rust_sdk::{
    MultisigConfig, SolanaLedgerMultisigTransaction as RustSolanaLedgerMultisigTransaction,
};
use wasm_bindgen::prelude::*;

use crate::client::WasmTradingApi;
use crate::errors::WasmResult;
use crate::transaction_builder::WasmUnsignedTransaction;
use crate::utils::to_fixed_bytes;

// ── WasmMultisigConfig ───────────────────────────────────────────────────────

/// An M-of-N multisig signer set.
///
/// Public keys are canonicalized (sorted bytewise) on construction, so the
/// same set of keys always produces the same credential id regardless of
/// input order.
#[wasm_bindgen(js_name = MultisigConfig)]
pub struct WasmMultisigConfig {
    pub(crate) inner: MultisigConfig,
}

#[wasm_bindgen(js_class = MultisigConfig)]
impl WasmMultisigConfig {
    /// Create a multisig config from a threshold and a set of Ed25519 public
    /// keys. Requires 2-21 distinct keys and `1 <= minSigners <= keys.length`.
    ///
    /// @param {number} minSigners - The signature threshold (the M in M-of-N).
    /// @param {Uint8Array[]} pubkeys - The signers' 32-byte public keys.
    /// @example
    /// ```js
    /// const config = new MultisigConfig(2, [pubkeyA, pubkeyB, pubkeyC]);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        min_signers: u8,
        pubkeys: Vec<js_sys::Uint8Array>,
    ) -> WasmResult<WasmMultisigConfig> {
        let pubkeys: Vec<[u8; 32]> = pubkeys
            .iter()
            .map(|key| to_fixed_bytes::<32>(&key.to_vec(), "public key"))
            .collect::<Result<_, _>>()?;
        Ok(WasmMultisigConfig {
            inner: MultisigConfig::new(min_signers, pubkeys)?,
        })
    }

    /// The signature threshold (the M in M-of-N).
    /// @returns {number}
    #[wasm_bindgen(js_name = minSigners)]
    pub fn min_signers(&self) -> u8 {
        self.inner.min_signers()
    }

    /// The signer public keys in canonical (bytewise sorted) order.
    /// @returns {Uint8Array[]}
    pub fn pubkeys(&self) -> Vec<js_sys::Uint8Array> {
        self.inner
            .pubkeys()
            .iter()
            .map(|key| js_sys::Uint8Array::from(key.as_slice()))
            .collect()
    }

    /// The 32-byte credential id of this multisig:
    /// `sha256(min_signers_u8 || borsh(sorted pubkeys))`.
    ///
    /// This is the account identity the rollup derives for the multisig.
    ///
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = credentialId)]
    pub fn credential_id(&self) -> Vec<u8> {
        self.inner.credential_id().to_vec()
    }

    /// The base58-encoded credential id, as committed to in the signed
    /// multisig message (`multisig_id` field).
    /// @returns {string}
    #[wasm_bindgen(js_name = multisigId)]
    pub fn multisig_id(&self) -> String {
        self.inner.multisig_id()
    }
}

// ── WasmUnsignedTransaction multisig methods ─────────────────────────────────

#[wasm_bindgen(js_class = UnsignedTransaction)]
impl WasmUnsignedTransaction {
    /// Serialize into the JSON message every multisig signer must commit to.
    ///
    /// Extends `toMessageBytes()` with the `multisig_id` (base58 credential
    /// id) and `version: 1` fields required by the V1 Solana offchain payload.
    ///
    /// @param {MultisigConfig} config - The multisig signer set.
    /// @returns {Uint8Array} UTF-8 JSON bytes.
    #[wasm_bindgen(js_name = toMultisigMessageBytes)]
    pub fn to_multisig_message_bytes(&self, config: &WasmMultisigConfig) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_multisig_message_bytes(&config.inner)?)
    }

    /// Build the bytes each multisig signer (e.g. a Ledger hardware wallet)
    /// must sign.
    ///
    /// Prepends the spec-compliant Solana off-chain preamble — carrying all N
    /// signer pubkeys — to the multisig JSON message. Equivalent to
    /// `new SolanaLedgerMultisigTransaction(unsigned, config).signableBytes()`.
    ///
    /// @param {MultisigConfig} config - The multisig signer set.
    /// @returns {Uint8Array} Bytes to pass to `wallet.signMessage`.
    #[wasm_bindgen(js_name = toLedgerMultisigSignableBytes)]
    pub fn to_ledger_multisig_signable_bytes(
        &self,
        config: &WasmMultisigConfig,
    ) -> WasmResult<Vec<u8>> {
        Ok(self
            .inner
            .to_ledger_multisig_signable_bytes(&config.inner)?)
    }
}

// ── WasmSolanaLedgerMultisigTransaction ──────────────────────────────────────

/// A multisig transaction collecting signatures over the spec-compliant Solana
/// offchain wire format (the format Ledger hardware wallets sign).
///
/// Every signer signs the same `signableBytes()`. Once `isComplete()`, submit
/// via `client.sendLedgerMultisigTransaction(tx)`.
#[wasm_bindgen(js_name = SolanaLedgerMultisigTransaction)]
pub struct WasmSolanaLedgerMultisigTransaction {
    pub(crate) inner: RustSolanaLedgerMultisigTransaction,
}

#[wasm_bindgen(js_class = SolanaLedgerMultisigTransaction)]
impl WasmSolanaLedgerMultisigTransaction {
    /// Create a multisig transaction from an unsigned transaction and the
    /// signer set.
    ///
    /// The signable bytes bake in the chain hash at construction time, so if
    /// the chain hash rotates (a schema update) before enough signatures are
    /// collected the submission fails and every signer must re-sign.
    ///
    /// Distribute `signableBytes()` to each signer rather than rebuilding the
    /// transaction independently — every signer must sign byte-identical input.
    ///
    /// @param {UnsignedTransaction} unsignedTx - The unsigned transaction.
    /// @param {MultisigConfig} config - The multisig signer set.
    /// @example
    /// ```js
    /// const unsigned = Transaction.builder()
    ///     .callMessage(callMsg)
    ///     .buildUnsigned(client);
    /// const tx = new SolanaLedgerMultisigTransaction(unsigned, config);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        unsigned_tx: WasmUnsignedTransaction,
        config: &WasmMultisigConfig,
    ) -> WasmResult<WasmSolanaLedgerMultisigTransaction> {
        Ok(WasmSolanaLedgerMultisigTransaction {
            inner: RustSolanaLedgerMultisigTransaction::new(
                unsigned_tx.inner,
                config.inner.clone(),
            )?,
        })
    }

    /// The bytes every signer must sign (preamble + JSON message).
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = signableBytes)]
    pub fn signable_bytes(&self) -> Vec<u8> {
        self.inner.signable_bytes().to_vec()
    }

    /// Add one signer's Ed25519 signature over `signableBytes()`.
    ///
    /// Rejects signers outside the configured set, duplicate signatures, and
    /// signatures that do not verify against the signable bytes.
    ///
    /// @param {Uint8Array} pubKey - The signer's 32-byte public key.
    /// @param {Uint8Array} signature - 64-byte Ed25519 signature.
    #[wasm_bindgen(js_name = addSignature)]
    pub fn add_signature(&mut self, pub_key: &[u8], signature: &[u8]) -> WasmResult<()> {
        let pub_key = to_fixed_bytes::<32>(pub_key, "public key")?;
        let signature = to_fixed_bytes::<64>(signature, "signature")?;
        Ok(self.inner.add_signature(pub_key, signature)?)
    }

    /// Number of signatures collected so far.
    /// @returns {number}
    #[wasm_bindgen(js_name = signatureCount)]
    pub fn signature_count(&self) -> usize {
        self.inner.signature_count()
    }

    /// Whether enough signatures have been collected to meet the threshold.
    /// @returns {boolean}
    #[wasm_bindgen(js_name = isComplete)]
    pub fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    /// Serialize to the raw binary wire format. Errors if the threshold has
    /// not been met.
    /// @returns {Uint8Array}
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        Ok(self.inner.to_bytes()?)
    }

    /// Serialize to wire format and base64-encode for submission.
    /// @returns {string}
    #[wasm_bindgen(js_name = toBase64)]
    pub fn to_base64(&self) -> WasmResult<String> {
        Ok(self.inner.to_base64()?)
    }
}

// ── Client convenience methods ───────────────────────────────────────────────

#[wasm_bindgen(js_class = Client)]
impl WasmTradingApi {
    /// Send a completed multisig transaction to the sequencer via REST.
    ///
    /// The transaction must have collected at least `minSigners` signatures.
    ///
    /// @param {SolanaLedgerMultisigTransaction} tx - A completed multisig transaction.
    /// @returns {Promise<SubmitTxResponse>}
    #[wasm_bindgen(js_name = sendLedgerMultisigTransaction)]
    pub async fn send_ledger_multisig_transaction(
        &self,
        tx: &WasmSolanaLedgerMultisigTransaction,
    ) -> WasmResult<crate::generated::WasmSubmitTxResponse> {
        let resp = self
            .inner
            .send_ledger_multisig_transaction(&tx.inner)
            .await?;
        Ok(crate::generated::WasmSubmitTxResponse(resp))
    }
}
