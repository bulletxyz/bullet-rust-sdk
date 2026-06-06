//! Fluent transaction builder for constructing and submitting transactions.
//!
//! All transaction construction goes through the builder pattern:
//!
//! ```ignore
//! use bullet_rust_sdk::{Transaction, UnsignedTransaction, Client, Keypair};
//!
//! // Build and send with explicit signer
//! let response = Transaction::builder()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .signer(&keypair)
//!     .client(&client)
//!     .build()?;
//!
//! // External signing
//! let unsigned = UnsignedTransaction::builder()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .client(&client)
//!     .build()?;
//!
//! let signable = unsigned.to_bytes()?;
//! let signature: [u8; 64] = external_signer.sign(&signable);
//! let pub_key: [u8; 32] = external_signer.public_key();
//! let signed = Transaction::from_parts(unsigned, signature, pub_key);
//!
//! // Submit later
//! client.send_transaction(&signed).await?;
//! ```

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bon::bon;
use borsh::{BorshDeserialize, BorshSerialize};
use bullet_exchange_interface::schema::Schema;
use bullet_exchange_interface::transaction::{
    Amount, Gas, PriorityFeeBips, RuntimeCall, Transaction as SignedTransaction, TxDetails,
    UniquenessData, UnsignedTransaction as RawUnsignedTransaction, Version0,
};
use serde_json::Value;

use crate::codegen::Error::ErrorResponse;
use crate::generated::types::{
    ApiErrorResponse, SubmitSolanaOffchainTxRequest, SubmitTxRequest, SubmitTxResponse,
};
use crate::types::CallMessage;
use crate::{Client, Keypair, SDKError, SDKResult};

// ── UnsignedTransaction ──────────────────────────────────────────────────────

/// An unsigned transaction with the chain hash baked in.
///
/// Created by [`UnsignedTransaction::build`]. Contains everything
/// needed to produce signable bytes without a client reference.
#[derive(Debug)]
pub struct UnsignedTransaction {
    pub(crate) inner: RawUnsignedTransaction,
    pub(crate) chain_hash: [u8; 32],
    pub(crate) chain_name: String,
}

#[bon]
impl UnsignedTransaction {
    /// Serialize into the bytes that must be signed.
    ///
    /// Borsh-serializes the transaction and appends the chain hash (32 bytes)
    /// as a domain separator.
    pub fn to_bytes(&self) -> SDKResult<Vec<u8>> {
        let mut data =
            borsh::to_vec(&self.inner).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        data.extend_from_slice(&self.chain_hash);
        Ok(data)
    }

    /// Reconstruct an [`UnsignedTransaction`] from the canonical bytes produced
    /// by [`to_bytes`](UnsignedTransaction::to_bytes) — the inverse of that
    /// method.
    ///
    /// `to_bytes` is `borsh(payload) ++ chain_hash` (a 32-byte domain
    /// separator). This reads back the payload and trailing chain hash, taking
    /// `chain_name` from `client`. The embedded chain hash is checked against
    /// the client's to reject bytes that were built for a different network.
    ///
    /// This lets a coordinator persist a transaction as its exact signable
    /// bytes and rebuild it later — across SDK upgrades or process restarts —
    /// without re-deriving from structured inputs. Because the payload is read
    /// back verbatim rather than re-serialized, the rebuilt signable bytes are
    /// byte-identical to what was signed; the stored bytes, not a separate JSON
    /// representation, are the source of truth.
    pub fn from_bytes(bytes: &[u8], client: &Client) -> SDKResult<UnsignedTransaction> {
        const CHAIN_HASH_LEN: usize = 32;
        if bytes.len() < CHAIN_HASH_LEN {
            return Err(SDKError::SerializationError(format!(
                "unsigned transaction bytes too short: {} (need at least {CHAIN_HASH_LEN} for the chain hash)",
                bytes.len()
            )));
        }
        let (payload, chain_hash_bytes) = bytes.split_at(bytes.len() - CHAIN_HASH_LEN);
        let mut chain_hash = [0u8; CHAIN_HASH_LEN];
        chain_hash.copy_from_slice(chain_hash_bytes);

        if chain_hash != client.chain_hash() {
            return Err(SDKError::InvalidChainHash(
                "does not match the connected client (built for a different network?)".to_string(),
            ));
        }

        let inner = RawUnsignedTransaction::try_from_slice(payload)
            .map_err(|e| SDKError::SerializationError(e.to_string()))?;

        Ok(UnsignedTransaction { inner, chain_hash, chain_name: client.chain_name() })
    }

    /// Render the unsigned transaction payload as a human-readable message.
    ///
    /// This is useful when an external wallet can only show raw message bytes
    /// during signing. The returned string is for display only; sign the bytes
    /// from [`to_bytes`](UnsignedTransaction::to_bytes).
    pub fn to_display_message(&self) -> SDKResult<String> {
        let schema = Schema::of_single_type::<RawUnsignedTransaction>()
            .map_err(|e| SDKError::SerializationError(e.to_string()))?;
        let bytes =
            borsh::to_vec(&self.inner).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        schema.display(0, &bytes).map_err(|e| SDKError::SerializationError(e.to_string()))
    }

    /// Build the bytes a Ledger hardware wallet must sign.
    ///
    /// Prepends the 85-byte Solana off-chain message preamble (using `chain_hash`
    /// as the application domain) to the JSON message bytes. This is the spec-
    /// compliant format required by Ledger firmware. Pass the resulting bytes to
    /// `wallet.signMessage`; then assemble and submit with
    /// [`SolanaLedgerTransaction::from_parts`] and [`Client::send_ledger_transaction`].
    pub fn to_ledger_signable_bytes(&self, pubkey: &[u8; 32]) -> SDKResult<Vec<u8>> {
        let json_bytes = self.to_message_bytes()?;
        let message_len = u16::try_from(json_bytes.len()).map_err(|_| {
            SDKError::SerializationError(format!(
                "JSON message too large for Solana preamble: {} bytes (max {})",
                json_bytes.len(),
                u16::MAX
            ))
        })?;
        let mut result = make_solana_preamble(&[*pubkey], &self.chain_hash, message_len);
        result.extend_from_slice(&json_bytes);
        Ok(result)
    }

    /// Serialize into the human-readable JSON bytes for offchain signing.
    ///
    /// This is the message external Solana wallets should sign when the backend
    /// uses the `solanaSimple` authenticator. The resulting signature must be
    /// wrapped with [`SolanaOffchainTransaction::from_parts`] and submitted via
    /// [`Client::send_offchain_transaction`].
    pub fn to_message_bytes(&self) -> SDKResult<Vec<u8>> {
        let message = serde_json::to_value(&self.inner)?;

        let Value::Object(mut message) = message else {
            return Err(SDKError::SerializationError(
                "unsigned transaction serialized to non-object JSON".to_string(),
            ));
        };
        // The Solana offchain authenticator expects only `max_fee` as a JSON
        // string today because it is the interface's only u128 transaction
        // detail. Other integer fields must remain JSON numbers.
        stringify_offchain_max_fee(&mut message, self.inner.details.max_fee.0)?;
        message.insert("chain_name".to_string(), Value::String(self.chain_name.clone()));
        serde_json::to_vec(&Value::Object(message)).map_err(Into::into)
    }

    /// Build an unsigned transaction.
    ///
    /// The returned [`UnsignedTransaction`] contains the chain hash, so
    /// [`to_bytes()`](UnsignedTransaction::to_bytes) produces signable bytes
    /// without needing a client reference.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = UnsignedTransaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .client(&client)
    ///     .build()?;
    ///
    /// let signable = unsigned.to_bytes()?;
    /// let signature: [u8; 64] = external_signer.sign(&signable);
    /// let pub_key: [u8; 32] = external_signer.public_key();
    /// let signed = Transaction::from_parts(unsigned, signature, pub_key);
    /// ```
    #[builder]
    pub fn new(
        call_message: CallMessage,
        max_fee: u128,
        priority_fee_bips: u64,
        gas_limit: Option<Gas>,
        /// Transaction uniqueness ([`UniquenessData::Nonce`],
        /// [`UniquenessData::Generation`], or [`UniquenessData::Window`]).
        ///
        /// Defaults to [`UniquenessData::Window`] from a per-client counter
        /// that tracks the millisecond unix timestamp and increments per
        /// transaction — a monotonic, duplicate-free value that needs no chain
        /// round-trip and tolerates many in-flight transactions. Set explicitly
        /// to use a nonce or generation instead.
        uniqueness: Option<UniquenessData>,
        client: &Client,
    ) -> SDKResult<UnsignedTransaction> {
        Self::from_runtime_call(
            RuntimeCall::Exchange(call_message),
            max_fee,
            priority_fee_bips,
            gas_limit,
            uniqueness,
            client,
        )
    }

    /// Build an unsigned transaction from a whole [`RuntimeCall`].
    ///
    /// This is the primary constructor: [`builder`](UnsignedTransaction::builder)
    /// is sugar that wraps a typed [`CallMessage`] as `RuntimeCall::Exchange` and
    /// calls this. Use this directly to build a call assembled dynamically — e.g.
    /// `RuntimeCall` deserialized from JSON via its `serde` support — without
    /// going through the typed factories.
    ///
    /// Exchange call messages are validated against the connected client's
    /// schema; other runtime-call variants pass through.
    pub fn from_runtime_call(
        runtime_call: RuntimeCall,
        max_fee: u128,
        priority_fee_bips: u64,
        gas_limit: Option<Gas>,
        uniqueness: Option<UniquenessData>,
        client: &Client,
    ) -> SDKResult<UnsignedTransaction> {
        if let RuntimeCall::Exchange(ref call_message) = runtime_call
            && !Client::call_message_was_validated(call_message, client.user_actions().as_deref())
        {
            return Err(SDKError::UnsupportedCallMessage(call_message.msg_type()));
        }

        let uniqueness =
            uniqueness.unwrap_or_else(|| UniquenessData::Window(client.next_window_nonce()));
        let details = TxDetails {
            chain_id: client.chain_id(),
            max_fee: Amount(max_fee),
            gas_limit,
            max_priority_fee_bips: PriorityFeeBips(priority_fee_bips),
        };

        Ok(UnsignedTransaction {
            inner: RawUnsignedTransaction { runtime_call, uniqueness, details },
            chain_hash: client.chain_hash(),
            chain_name: client.chain_name(),
        })
    }
}

// ── Solana preamble ──────────────────────────────────────────────────────────

// Solana off-chain signing domain: 0xff followed by "solana offchain" (15 bytes).
const SOLANA_SIGNING_DOMAIN: [u8; 16] = *b"\xffsolana offchain";

/// Build the Solana off-chain message preamble for one or more signers.
///
/// A single-signer preamble is just `N = 1`. Layout
/// (`53 + 32 * pubkeys.len()` bytes):
///   `[16: signing domain][1: header_version][32: chain_hash][1: message_format]
///    [1: signer_count = N][32 * N: pubkeys][2: message_length LE]`
pub(crate) fn make_solana_preamble(
    pubkeys: &[[u8; 32]],
    chain_hash: &[u8; 32],
    message_length: u16,
) -> Vec<u8> {
    let mut preamble = Vec::with_capacity(53 + 32 * pubkeys.len());
    preamble.extend_from_slice(&SOLANA_SIGNING_DOMAIN);
    preamble.push(0); // header_version
    preamble.extend_from_slice(chain_hash);
    preamble.push(0); // message_format
    preamble.push(pubkeys.len() as u8); // signer_count
    for pubkey in pubkeys {
        preamble.extend_from_slice(pubkey);
    }
    preamble.extend_from_slice(&message_length.to_le_bytes());
    preamble
}

// ── Solana offchain transaction ──────────────────────────────────────────────

/// A Solana offchain transaction ready for sequencer submission.
///
/// Use this for external Solana wallets that should display readable JSON
/// instead of Borsh bytes. Sign [`UnsignedTransaction::to_message_bytes`],
/// assemble with [`SolanaOffchainTransaction::from_parts`], then submit via
/// [`Client::send_offchain_transaction`].
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct SolanaOffchainTransaction {
    /// JSON message bytes signed by the Solana wallet.
    pub signed_message: Vec<u8>,
    /// Chain hash required by the current Solana offchain Borsh envelope.
    pub chain_hash: [u8; 32],
    /// Solana Ed25519 public key.
    pub pubkey: [u8; 32],
    /// Ed25519 signature over `signed_message`.
    pub signature: [u8; 64],
}

impl SolanaOffchainTransaction {
    /// Assemble a Solana offchain transaction from an unsigned transaction, a
    /// 64-byte Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing the bytes from
    /// [`UnsignedTransaction::to_message_bytes`].
    pub fn from_parts(
        tx: UnsignedTransaction,
        signature: [u8; 64],
        pubkey: [u8; 32],
    ) -> SDKResult<Self> {
        Ok(Self {
            signed_message: tx.to_message_bytes()?,
            chain_hash: tx.chain_hash,
            pubkey,
            signature,
        })
    }

    /// Borsh-serialize a Solana offchain transaction to bytes.
    pub fn to_bytes(&self) -> SDKResult<Vec<u8>> {
        borsh::to_vec(self).map_err(|e| SDKError::SerializationError(e.to_string()))
    }

    /// Borsh-serialize and base64-encode a Solana offchain transaction.
    pub fn to_base64(&self) -> SDKResult<String> {
        let bytes = self.to_bytes()?;
        Ok(BASE64.encode(&bytes))
    }
}

// ── Solana Ledger transaction ────────────────────────────────────────────────

/// A Solana offchain transaction using the spec-compliant Ledger wire format.
///
/// Use this for Ledger hardware wallets. Sign the bytes from
/// [`UnsignedTransaction::to_ledger_signable_bytes`], assemble with
/// [`SolanaLedgerTransaction::from_parts`], then submit via
/// [`Client::send_ledger_transaction`].
///
/// Wire format: `[u32 LE: message_len][preamble + json][64-byte sig]`, base64-
/// encoded and posted as `{"body":"..."}`.
#[derive(Clone, Debug)]
pub struct SolanaLedgerTransaction {
    /// Preamble (85 bytes) concatenated with JSON message bytes.
    pub signed_message: Vec<u8>,
    /// Ed25519 signature over `signed_message`.
    pub signature: [u8; 64],
}

impl SolanaLedgerTransaction {
    /// Assemble a Ledger transaction from an unsigned transaction, a 32-byte
    /// public key, and a 64-byte Ed25519 signature.
    ///
    /// Use after signing [`UnsignedTransaction::to_ledger_signable_bytes`].
    pub fn from_parts(
        tx: UnsignedTransaction,
        pubkey: [u8; 32],
        signature: [u8; 64],
    ) -> SDKResult<Self> {
        Ok(Self { signed_message: tx.to_ledger_signable_bytes(&pubkey)?, signature })
    }

    /// Serialize to the raw binary wire format.
    ///
    /// Layout: `[u32 LE: message_len][signed_message][signature]`
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.signed_message.len() + 64);
        buf.extend_from_slice(&(self.signed_message.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.signed_message);
        buf.extend_from_slice(&self.signature);
        buf
    }

    /// Serialize to wire format and base64-encode.
    pub fn to_base64(&self) -> String {
        BASE64.encode(self.to_bytes())
    }
}

// ── Transaction ──────────────────────────────────────────────────────────────

/// Transaction construction and serialization.
///
/// Use `Transaction::builder()` for signed transactions, or
/// `UnsignedTransaction::builder()` for external signing.
pub struct Transaction;

#[bon]
impl Transaction {
    /// Build a signed transaction.
    ///
    /// Internally builds an unsigned transaction, serializes it,
    /// signs with the provided keypair, and assembles the result.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let signed = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .signer(&keypair)
    ///     .client(&client)
    ///     .build()?;
    ///
    /// client.send_transaction(&signed).await?;
    /// ```
    #[builder]
    pub fn new(
        call_message: CallMessage,
        max_fee: Option<u128>,
        priority_fee_bips: Option<u64>,
        gas_limit: Option<Gas>,
        uniqueness: Option<UniquenessData>,
        signer: Option<&Keypair>,
        client: &Client,
    ) -> SDKResult<SignedTransaction> {
        Self::from_runtime_call(
            RuntimeCall::Exchange(call_message),
            max_fee,
            priority_fee_bips,
            gas_limit,
            uniqueness,
            signer,
            client,
        )
    }

    /// Build and sign a transaction from a whole [`RuntimeCall`].
    ///
    /// The signed-transaction counterpart of
    /// [`UnsignedTransaction::from_runtime_call`]: builds the unsigned
    /// transaction, signs `to_bytes()` with `signer` (falling back to the
    /// client's keypair), and assembles the result. Use this to sign a call
    /// assembled dynamically rather than via the typed factories.
    #[allow(clippy::too_many_arguments)]
    pub fn from_runtime_call(
        runtime_call: RuntimeCall,
        max_fee: Option<u128>,
        priority_fee_bips: Option<u64>,
        gas_limit: Option<Gas>,
        uniqueness: Option<UniquenessData>,
        signer: Option<&Keypair>,
        client: &Client,
    ) -> SDKResult<SignedTransaction> {
        let signer = signer.or_else(|| client.keypair()).ok_or(SDKError::MissingKeypair)?;

        let max_fee = max_fee.unwrap_or_else(|| client.max_fee().0);
        let priority_fee_bips =
            priority_fee_bips.unwrap_or_else(|| client.max_priority_fee_bips().0);
        let gas_limit = gas_limit.or_else(|| client.gas_limit());

        let unsigned = UnsignedTransaction::from_runtime_call(
            runtime_call,
            max_fee,
            priority_fee_bips,
            gas_limit,
            uniqueness,
            client,
        )?;

        let data = unsigned.to_bytes()?;
        let sig_bytes: [u8; 64] = signer
            .sign(&data)
            .try_into()
            .map_err(|v: Vec<u8>| SDKError::InvalidSignatureLength(v.len()))?;
        let pub_key: [u8; 32] = signer
            .public_key()
            .try_into()
            .map_err(|v: Vec<u8>| SDKError::InvalidPublicKeyLength(v.len()))?;

        Ok(Self::from_parts(unsigned, sig_bytes, pub_key))
    }

    /// Assemble a signed transaction from an unsigned transaction, a 64-byte
    /// Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing the bytes from [`UnsignedTransaction::to_bytes`].
    pub fn from_parts(
        tx: UnsignedTransaction,
        signature: [u8; 64],
        pub_key: [u8; 32],
    ) -> SignedTransaction {
        let RawUnsignedTransaction { runtime_call, uniqueness, details } = tx.inner;
        SignedTransaction::V0(Version0 { runtime_call, uniqueness, details, pub_key, signature })
    }

    /// Borsh-serialize a signed transaction to bytes.
    ///
    /// Useful for byte-level comparison of two signed transactions.
    pub fn to_bytes(signed: &SignedTransaction) -> SDKResult<Vec<u8>> {
        borsh::to_vec(signed).map_err(|e| SDKError::SerializationError(e.to_string()))
    }

    /// Borsh-serialize and base64-encode a signed transaction.
    pub fn to_base64(signed: &SignedTransaction) -> SDKResult<String> {
        let bytes = Self::to_bytes(signed)?;
        Ok(BASE64.encode(&bytes))
    }
}

// ── Client methods ───────────────────────────────────────────────────────────
impl Client {
    /// Send a signed transaction to the network.
    ///
    /// Returns the response from the sequencer.
    pub async fn send_transaction(
        &self,
        signed: &SignedTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        let body = Transaction::to_base64(signed)?;
        let response = self.client().submit_tx(&SubmitTxRequest { body }).await;
        match response {
            Err(ErrorResponse(response)) if response.status() == 401 => {
                let inner = response.into_inner();
                Err(self.submit_tx_api_error(inner).await?)
            }
            Ok(r) => Ok(r.into_inner()),
            Err(e) => Err(e.into()),
        }
    }

    /// Send a Solana offchain transaction to the network.
    ///
    /// Submits to the trading API's `/api/v1/solanaOffchainTx`, which accepts
    /// the JSON-based Solana authenticator payload used by external Solana
    /// wallets and proxies it to the rollup.
    pub async fn send_offchain_transaction(
        &self,
        signed: &SolanaOffchainTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        self.submit_offchain(signed.to_base64()?).await
    }

    /// Send a Solana Ledger transaction to the network.
    ///
    /// Submits the spec-compliant wire format to the trading API's
    /// `/api/v1/solanaOffchainTx`. Use after signing with
    /// [`UnsignedTransaction::to_ledger_signable_bytes`] and assembling via
    /// [`SolanaLedgerTransaction::from_parts`].
    pub async fn send_ledger_transaction(
        &self,
        signed: &SolanaLedgerTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        self.submit_offchain(signed.to_base64()).await
    }

    /// Submit a base64-encoded Solana offchain envelope via the trading API.
    ///
    /// Stale-chain-hash errors are mapped to [`SDKError::TransactionOutdated`]
    /// so the caller knows to rebuild and re-sign — for the offchain envelope
    /// that's the spec's `400` chain-hash mismatch (the chain hash is a
    /// validated field), as well as a `401` invalid-signature.
    pub(crate) async fn submit_offchain(&self, body: String) -> SDKResult<SubmitTxResponse> {
        let request = SubmitSolanaOffchainTxRequest { body };
        match self.client().submit_solana_offchain_tx(&request).await {
            Ok(r) => Ok(r.into_inner()),
            Err(ErrorResponse(response)) => {
                Err(self.submit_tx_api_error(response.into_inner()).await?)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Build, sign, and submit a call message, retrying once if the chain hash
    /// changed since startup (401 invalid signature → schema refresh → re-sign).
    ///
    /// Unlike calling `send_transaction` directly, this never returns
    /// `TransactionOutdated` — the retry is handled internally, and if the
    /// refreshed hash also fails the error is returned as `ApiError`.
    pub async fn send_call_message(
        &self,
        call_message: CallMessage,
    ) -> SDKResult<SubmitTxResponse> {
        let signed =
            Transaction::builder().call_message(call_message.clone()).client(self).build()?;
        match self.send_transaction(&signed).await {
            Err(SDKError::TransactionOutdated) => {
                // chain hash was refreshed; re-sign with the new hash and retry once.
                // submit directly so a second 401 comes back as ApiError, not TransactionOutdated
                let signed =
                    Transaction::builder().call_message(call_message).client(self).build()?;
                let body = Transaction::to_base64(&signed)?;
                match self.client().submit_tx(&SubmitTxRequest { body }).await {
                    Ok(r) => Ok(r.into_inner()),
                    Err(ErrorResponse(r)) => Err(SDKError::ApiError(Box::new(r.into_inner()))),
                    Err(e) => Err(e.into()),
                }
            }
            other => other,
        }
    }

    async fn submit_tx_api_error(&self, error: ApiErrorResponse) -> SDKResult<SDKError> {
        // A stale chain hash surfaces differently per submission path: the borsh
        // path bakes it into the signed bytes (→ 401 invalid signature), while
        // the Solana offchain envelope carries it as a validated field (→ 400
        // chain-hash mismatch). Both mean "rebuild and re-sign".
        let stale_chain_hash = (error.status == 401 && error.message.contains("Invalid signature"))
            || (error.status == 400 && {
                let message = error.message.to_lowercase();
                message.contains("chain_hash mismatch") || message.contains("chain hash mismatch")
            });
        if stale_chain_hash {
            self.update_schema().await?;
            return Ok(SDKError::TransactionOutdated);
        }
        Ok(SDKError::ApiError(Box::new(error)))
    }
}

fn stringify_offchain_max_fee(
    message: &mut serde_json::Map<String, Value>,
    max_fee: u128,
) -> SDKResult<()> {
    let details = message.get_mut("details").and_then(Value::as_object_mut).ok_or_else(|| {
        SDKError::SerializationError(
            "unsigned transaction JSON missing object field details".to_string(),
        )
    })?;
    let max_fee_value = details.get_mut("max_fee").ok_or_else(|| {
        SDKError::SerializationError(
            "unsigned transaction JSON missing field details.max_fee".to_string(),
        )
    })?;
    *max_fee_value = Value::String(max_fee.to_string());
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use bullet_exchange_interface::message::{CancelOrderArgs, PublicAction, UserAction};
    use bullet_exchange_interface::schema::Schema;
    use bullet_exchange_interface::transaction::{
        Amount, PriorityFeeBips, RuntimeCall, Transaction as InterfaceTransaction, TxDetails,
        UniquenessData,
    };
    use bullet_exchange_interface::types::{MarketId, OrderId};
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn test_unsigned_tx() -> UnsignedTransaction {
        let inner = RawUnsignedTransaction {
            runtime_call: RuntimeCall::Exchange(CallMessage::Public(PublicAction::ApplyFunding {
                addresses: vec![],
            })),
            uniqueness: UniquenessData::Generation(12345),
            details: TxDetails {
                chain_id: 1,
                max_fee: Amount(10_000_000),
                gas_limit: None,
                max_priority_fee_bips: PriorityFeeBips(0),
            },
        };
        UnsignedTransaction { inner, chain_hash: [42u8; 32], chain_name: "TestChain".to_string() }
    }

    fn schema_response(chain_byte: u8) -> serde_json::Value {
        let schema = Schema::of_single_type::<InterfaceTransaction>().unwrap();
        serde_json::json!({
            "chain_hash": format!("0x{}", hex::encode([chain_byte; 32])),
            "schema": schema,
        })
    }

    fn exchange_info_response() -> serde_json::Value {
        serde_json::json!({
            "assets": [],
            "rateLimits": [],
            "symbols": [],
            "globalConfig": {
                "maxOrdersPerUser": 0,
                "maxTriggerOrdersPerUser": 0,
                "maxTriggerOrdersToExecutePerMsg": 0,
                "minNotionalTwapValue": "0",
                "minNotionalTwapValuePerOrder": "0",
                "twapExecutionIntervalSeconds": 0,
            },
        })
    }

    async fn mock_client_for_offchain_submission(
        offchain_response: ResponseTemplate,
    ) -> (MockServer, Client) {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rollup/schema"))
            .respond_with(ResponseTemplate::new(200).set_body_json(schema_response(7)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/fapi/v1/exchangeInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(exchange_info_response()))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/solanaOffchainTx"))
            .respond_with(offchain_response)
            .mount(&server)
            .await;

        let client = Client::builder().network(server.uri()).build().await.unwrap();

        (server, client)
    }

    fn test_solana_offchain_transaction() -> SolanaOffchainTransaction {
        let chain_hash = [7u8; 32];
        SolanaOffchainTransaction {
            signed_message: serde_json::json!({
                "chain_name": "TestChain",
            })
            .to_string()
            .into_bytes(),
            chain_hash,
            pubkey: [8u8; 32],
            signature: [9u8; 64],
        }
    }

    fn verify_signature(pub_key: [u8; 32], message: &[u8], signature: [u8; 64]) -> bool {
        let verifying_key = VerifyingKey::from_bytes(&pub_key).unwrap();
        let signature = Signature::from_bytes(&signature);
        verifying_key.verify(message, &signature).is_ok()
    }

    #[test]
    fn to_bytes_is_borsh_plus_chain_hash() {
        let unsigned = test_unsigned_tx();
        let bytes = unsigned.to_bytes().unwrap();

        let mut expected = borsh::to_vec(&unsigned.inner).unwrap();
        expected.extend_from_slice(&unsigned.chain_hash);
        assert_eq!(bytes, expected);
    }

    #[tokio::test]
    async fn from_bytes_roundtrips_to_bytes() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;

        // schema_response(7) sets the client chain hash to [7u8; 32].
        let inner = RawUnsignedTransaction {
            runtime_call: RuntimeCall::Exchange(CallMessage::Public(PublicAction::ApplyFunding {
                addresses: vec![],
            })),
            uniqueness: UniquenessData::Window(99),
            details: TxDetails {
                chain_id: client.chain_id(),
                max_fee: Amount(10_000_000),
                gas_limit: None,
                max_priority_fee_bips: PriorityFeeBips(0),
            },
        };
        let original = UnsignedTransaction {
            inner,
            chain_hash: client.chain_hash(),
            chain_name: client.chain_name(),
        };
        let bytes = original.to_bytes().unwrap();

        let restored = UnsignedTransaction::from_bytes(&bytes, &client).unwrap();

        // Re-serializing the reconstructed tx yields byte-identical signable bytes.
        assert_eq!(restored.to_bytes().unwrap(), bytes);
        assert_eq!(restored.chain_hash, client.chain_hash());
        assert_eq!(restored.chain_name, client.chain_name());
    }

    #[tokio::test]
    async fn from_bytes_rejects_chain_hash_for_another_network() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;

        // Bytes whose trailing chain hash isn't the client's ([7u8; 32]).
        let mut foreign = test_unsigned_tx();
        foreign.chain_hash = [9u8; 32];
        let bytes = foreign.to_bytes().unwrap();

        let err = UnsignedTransaction::from_bytes(&bytes, &client).unwrap_err();
        assert!(matches!(err, SDKError::InvalidChainHash(_)), "{err:?}");
    }

    #[tokio::test]
    async fn from_bytes_rejects_truncated_input() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;

        let err = UnsignedTransaction::from_bytes(&[0u8; 8], &client).unwrap_err();
        assert!(matches!(err, SDKError::SerializationError(_)), "{err:?}");
    }

    #[test]
    fn to_display_message_renders_unsigned_payload_without_chain_hash() {
        let unsigned = test_unsigned_tx();

        let display = unsigned.to_display_message().unwrap();

        assert!(display.contains("ApplyFunding"), "{display}");
        assert!(display.contains("max_fee"), "{display}");
        assert!(display.contains("10000000"), "{display}");
        assert!(!display.contains("signature"), "{display}");
    }

    #[test]
    fn to_message_bytes_serializes_readable_json() {
        let unsigned = test_unsigned_tx();

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["chain_name"], "TestChain");
        assert_eq!(message["uniqueness"]["generation"], 12345);
        assert_eq!(message["details"]["chain_id"], 1);
        assert_eq!(message["details"]["max_fee"], "10000000");
        assert!(message.get("runtime_call").is_some());
    }

    #[test]
    fn to_message_bytes_omits_envelope_chain_hash() {
        let unsigned = test_unsigned_tx();

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert!(message.get("chain_hash").is_none());
    }

    #[test]
    fn to_message_bytes_serializes_large_max_fee_as_string() {
        let mut unsigned = test_unsigned_tx();
        let max_fee = u128::from(u64::MAX) + 1;
        unsigned.inner.details.max_fee = Amount(max_fee);

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["details"]["max_fee"], max_fee.to_string());
    }

    #[test]
    fn stringify_offchain_max_fee_errors_when_details_shape_changes() {
        let mut message = serde_json::json!({
            "runtime_call": {},
        })
        .as_object()
        .unwrap()
        .clone();

        let err = stringify_offchain_max_fee(&mut message, 1).unwrap_err();

        assert!(err.to_string().contains("missing object field details"));
    }

    #[test]
    fn stringify_offchain_max_fee_errors_when_max_fee_is_missing() {
        let mut message = serde_json::json!({
            "details": {
                "chain_id": 1,
            },
        })
        .as_object()
        .unwrap()
        .clone();

        let err = stringify_offchain_max_fee(&mut message, 1).unwrap_err();

        assert!(err.to_string().contains("missing field details.max_fee"));
    }

    #[test]
    fn to_message_bytes_matches_solana_authenticator_json_numbers() {
        let mut unsigned = test_unsigned_tx();
        unsigned.inner.details.max_priority_fee_bips = PriorityFeeBips(u64::MAX);

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["details"]["max_priority_fee_bips"], u64::MAX);
        assert_eq!(message["details"]["chain_id"], 1);
        assert_eq!(message["uniqueness"]["generation"], 12345);
    }

    #[test]
    fn to_message_bytes_serializes_order_ids_as_json_numbers() {
        let inner = RawUnsignedTransaction {
            runtime_call: RuntimeCall::Exchange(CallMessage::User(UserAction::CancelOrders {
                market_id: MarketId(0),
                orders: vec![CancelOrderArgs {
                    order_id: Some(OrderId(10_000_000_000)),
                    client_order_id: None,
                }],
                sub_account_index: None,
            })),
            uniqueness: UniquenessData::Generation(12345),
            details: TxDetails {
                chain_id: 1,
                max_fee: Amount(10_000_000),
                gas_limit: None,
                max_priority_fee_bips: PriorityFeeBips(0),
            },
        };
        let unsigned = UnsignedTransaction {
            inner,
            chain_hash: [42u8; 32],
            chain_name: "TestChain".to_string(),
        };

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(
            message["runtime_call"]["exchange"]["user"]["cancel_orders"]["orders"][0]["order_id"],
            10_000_000_000u64
        );
    }

    #[test]
    fn make_solana_preamble_layout() {
        let pubkey = [1u8; 32];
        let chain_hash = [2u8; 32];
        let message_length: u16 = 0x1234;

        let preamble = make_solana_preamble(&[pubkey], &chain_hash, message_length);

        assert_eq!(preamble.len(), 85);
        // signing domain: 0xff + "solana offchain"
        assert_eq!(preamble[0], 0xff);
        assert_eq!(&preamble[1..16], b"solana offchain");
        // header_version = 0
        assert_eq!(preamble[16], 0);
        // application_domain = chain_hash
        assert_eq!(&preamble[17..49], &chain_hash);
        // message_format = 0
        assert_eq!(preamble[49], 0);
        // signer_count = 1
        assert_eq!(preamble[50], 1);
        // pubkey
        assert_eq!(&preamble[51..83], &pubkey);
        // message_length LE
        assert_eq!(&preamble[83..85], &message_length.to_le_bytes());
    }

    #[test]
    fn to_ledger_signable_bytes_errors_when_json_exceeds_u16() {
        let pub_key = [1u8; 32];
        let mut unsigned = test_unsigned_tx();
        // Inject a payload large enough to exceed u16::MAX bytes when serialized
        unsigned.inner.runtime_call =
            RuntimeCall::Exchange(CallMessage::Public(PublicAction::ApplyFunding {
                addresses: vec![
                    bullet_exchange_interface::address::Address([0u8; 32]);
                    3000 // ~3000 * ~44 bytes each ≈ 132 KB > 65535
                ],
            }));

        let err = unsigned.to_ledger_signable_bytes(&pub_key).unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn to_ledger_signable_bytes_is_preamble_plus_json() {
        let keypair = Keypair::generate();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();
        let unsigned = test_unsigned_tx();

        let json_bytes = unsigned.to_message_bytes().unwrap();
        let signable = unsigned.to_ledger_signable_bytes(&pub_key).unwrap();

        assert_eq!(signable.len(), 85 + json_bytes.len());
        assert_eq!(&signable[85..], json_bytes.as_slice());
        // Preamble starts with signing domain
        assert_eq!(signable[0], 0xff);
        assert_eq!(&signable[1..16], b"solana offchain");
    }

    #[test]
    fn solana_ledger_transaction_wire_format() {
        let keypair = Keypair::generate();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_ledger_signable_bytes(&pub_key).unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();

        let tx =
            SolanaLedgerTransaction::from_parts(test_unsigned_tx(), pub_key, signature).unwrap();

        let wire = tx.to_bytes();
        // first 4 bytes: LE u32 message length
        let len = u32::from_le_bytes(wire[0..4].try_into().unwrap()) as usize;
        assert_eq!(len, tx.signed_message.len());
        assert_eq!(&wire[4..4 + len], tx.signed_message.as_slice());
        assert_eq!(&wire[4 + len..], &signature);
    }

    #[test]
    fn solana_ledger_signature_verifies_against_signable_bytes() {
        let keypair = Keypair::generate();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_ledger_signable_bytes(&pub_key).unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();

        let tx =
            SolanaLedgerTransaction::from_parts(test_unsigned_tx(), pub_key, signature).unwrap();

        assert!(verify_signature(pub_key, &tx.signed_message, tx.signature));
        // Must NOT verify against plain JSON bytes
        let json_bytes = test_unsigned_tx().to_message_bytes().unwrap();
        assert!(!verify_signature(pub_key, &json_bytes, tx.signature));
    }

    #[tokio::test]
    async fn send_ledger_transaction_requires_resign_on_invalid_signature() {
        let (server, client) = mock_client_for_offchain_submission(
            ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "status": 401,
                "message": "Invalid signature",
            })),
        )
        .await;

        let keypair = Keypair::generate();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();
        let unsigned = test_unsigned_tx();
        let signable = unsigned.to_ledger_signable_bytes(&pub_key).unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let tx =
            SolanaLedgerTransaction::from_parts(test_unsigned_tx(), pub_key, signature).unwrap();

        let err = client.send_ledger_transaction(&tx).await.unwrap_err();

        assert!(matches!(err, SDKError::TransactionOutdated), "{err:?}");
        let requests = server.received_requests().await.unwrap();
        let schema_requests =
            requests.iter().filter(|request| request.url.path() == "/rollup/schema").count();
        assert_eq!(schema_requests, 2);
    }

    #[tokio::test]
    async fn send_ledger_transaction_maps_chain_hash_mismatch_to_outdated() {
        // The offchain envelope carries the chain hash as a validated field, so
        // a stale hash comes back as a 400 (not a 401) — still "rebuild & re-sign".
        let (server, client) = mock_client_for_offchain_submission(
            ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "status": 400,
                "message": "chain_hash mismatch",
            })),
        )
        .await;

        let keypair = Keypair::generate();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();
        let signable = test_unsigned_tx().to_ledger_signable_bytes(&pub_key).unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let tx =
            SolanaLedgerTransaction::from_parts(test_unsigned_tx(), pub_key, signature).unwrap();

        let err = client.send_ledger_transaction(&tx).await.unwrap_err();

        assert!(matches!(err, SDKError::TransactionOutdated), "{err:?}");
        let requests = server.received_requests().await.unwrap();
        let schema_requests =
            requests.iter().filter(|request| request.url.path() == "/rollup/schema").count();
        assert_eq!(schema_requests, 2);
    }

    #[tokio::test]
    async fn signed_builder_defaults_to_window_uniqueness() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;
        let keypair = Keypair::generate();

        let signed = Transaction::builder()
            .call_message(CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] }))
            .max_fee(10_000_000)
            .signer(&keypair)
            .client(&client)
            .build()
            .unwrap();

        let SignedTransaction::V0(version_0) = signed else {
            panic!("expected V0 signed transaction");
        };
        assert!(
            matches!(version_0.uniqueness, UniquenessData::Window(_)),
            "{:?}",
            version_0.uniqueness
        );
    }

    #[tokio::test]
    async fn send_ledger_multisig_transaction_posts_base64_envelope() {
        let (server, client) = mock_client_for_offchain_submission(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "0xabc",
                "status": "submitted",
            })),
        )
        .await;

        let mut keypairs: Vec<Keypair> = (0..3).map(|_| Keypair::generate()).collect();
        keypairs.sort_by_key(|kp| kp.public_key());
        let pubkeys: Vec<[u8; 32]> =
            keypairs.iter().map(|kp| kp.public_key().try_into().unwrap()).collect();
        let config = crate::multisig::MultisigConfig::new(2, pubkeys).unwrap();

        let mut tx =
            crate::multisig::SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config)
                .unwrap();
        for keypair in &keypairs[..2] {
            let pubkey: [u8; 32] = keypair.public_key().try_into().unwrap();
            let signature: [u8; 64] = keypair.sign(tx.signable_bytes()).try_into().unwrap();
            tx.add_signature(pubkey, signature).unwrap();
        }

        let response = client.send_ledger_multisig_transaction(&tx).await.unwrap();

        assert_eq!(response.id, "0xabc");
        let requests = server.received_requests().await.unwrap();
        let submit = requests
            .iter()
            .find(|request| request.url.path() == "/api/v1/solanaOffchainTx")
            .expect("expected a submission request");
        let body: serde_json::Value = serde_json::from_slice(&submit.body).unwrap();
        assert_eq!(body["body"], tx.to_base64().unwrap());
    }

    #[test]
    fn solana_offchain_transaction_wraps_signed_json_message() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_message_bytes().unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();

        let tx = SolanaOffchainTransaction::from_parts(unsigned, signature, pub_key).unwrap();

        assert_eq!(tx.signed_message, signable);
        assert_eq!(tx.chain_hash, [42u8; 32]);
        assert_eq!(tx.pubkey, pub_key);
        assert_eq!(tx.signature, signature);
        assert!(!tx.to_base64().unwrap().is_empty());

        let roundtrip: SolanaOffchainTransaction =
            borsh::from_slice(&tx.to_bytes().unwrap()).expect("should deserialize");
        assert_eq!(roundtrip, tx);
    }

    #[test]
    fn signed_transaction_signature_verifies_against_borsh_bytes() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_bytes().unwrap();
        let signature: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();

        let signed = Transaction::from_parts(unsigned, signature, pub_key);
        let version_0 = match signed {
            SignedTransaction::V0(version_0) => version_0,
            _ => panic!("expected Version0 signed transaction"),
        };

        assert!(verify_signature(version_0.pub_key, &signable, version_0.signature));
    }

    #[test]
    fn solana_offchain_signature_verifies_against_message_bytes_only() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let message = unsigned.to_message_bytes().unwrap();
        let borsh_bytes = unsigned.to_bytes().unwrap();
        let signature: [u8; 64] = keypair.sign(&message).try_into().unwrap();
        let pub_key: [u8; 32] = keypair.public_key().try_into().unwrap();

        let tx = SolanaOffchainTransaction::from_parts(unsigned, signature, pub_key).unwrap();

        assert!(verify_signature(tx.pubkey, &tx.signed_message, tx.signature));
        assert!(!verify_signature(tx.pubkey, &borsh_bytes, tx.signature));
    }

    #[tokio::test]
    async fn send_offchain_transaction_requires_resign_on_invalid_signature() {
        let (server, client) = mock_client_for_offchain_submission(
            ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "status": 401,
                "message": "Invalid signature",
            })),
        )
        .await;
        let signed = test_solana_offchain_transaction();

        let err = client.send_offchain_transaction(&signed).await.unwrap_err();

        assert!(matches!(err, SDKError::TransactionOutdated), "{err:?}");
        let requests = server.received_requests().await.unwrap();
        let schema_requests =
            requests.iter().filter(|request| request.url.path() == "/rollup/schema").count();
        assert_eq!(schema_requests, 2);
    }

    #[tokio::test]
    async fn builder_uniqueness_nonce_and_window_serialize_to_json() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let unsigned = UnsignedTransaction::builder()
            .call_message(call_msg.clone())
            .max_fee(10_000_000)
            .priority_fee_bips(0)
            .uniqueness(UniquenessData::Nonce(7))
            .client(&client)
            .build()
            .unwrap();
        let message: serde_json::Value =
            serde_json::from_slice(&unsigned.to_message_bytes().unwrap()).unwrap();
        assert_eq!(message["uniqueness"]["nonce"], 7);

        let unsigned = UnsignedTransaction::builder()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .priority_fee_bips(0)
            .uniqueness(UniquenessData::Window(99))
            .client(&client)
            .build()
            .unwrap();
        let message: serde_json::Value =
            serde_json::from_slice(&unsigned.to_message_bytes().unwrap()).unwrap();
        assert_eq!(message["uniqueness"]["window"], 99);
    }

    #[tokio::test]
    async fn builder_defaults_to_window_uniqueness() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let unsigned = UnsignedTransaction::builder()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .priority_fee_bips(0)
            .client(&client)
            .build()
            .unwrap();
        let message: serde_json::Value =
            serde_json::from_slice(&unsigned.to_message_bytes().unwrap()).unwrap();
        assert!(message["uniqueness"]["window"].is_u64(), "{message}");
    }

    #[tokio::test]
    async fn builder_default_window_nonce_increments_per_tx() {
        let (_server, client) =
            mock_client_for_offchain_submission(ResponseTemplate::new(200)).await;
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let window_of = |client: &Client| -> u64 {
            let unsigned = UnsignedTransaction::builder()
                .call_message(call_msg.clone())
                .max_fee(10_000_000)
                .priority_fee_bips(0)
                .client(client)
                .build()
                .unwrap();
            match unsigned.inner.uniqueness {
                UniquenessData::Window(w) => w,
                other => panic!("expected Window, got {other:?}"),
            }
        };

        let first = window_of(&client);
        let second = window_of(&client);
        assert_eq!(second, first + 1, "window nonce must monotonically increment");
    }

    #[test]
    fn from_parts_matches_direct_construction() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_bytes().unwrap();
        let sig: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let pk: [u8; 32] = keypair.public_key().try_into().unwrap();

        // Reconstruct for direct comparison (from_parts consumes unsigned)
        let chain_hash = unsigned.chain_hash;
        let inner_clone = RawUnsignedTransaction {
            runtime_call: unsigned.inner.runtime_call.clone(),
            uniqueness: unsigned.inner.uniqueness.clone(),
            details: unsigned.inner.details.clone(),
        };
        let assembled = Transaction::from_parts(unsigned, sig, pk);

        // Direct Version0 construction
        let mut data = borsh::to_vec(&inner_clone).unwrap();
        data.extend_from_slice(&chain_hash);
        let sig2: [u8; 64] = keypair.sign(&data).try_into().unwrap();
        let direct = SignedTransaction::V0(Version0 {
            runtime_call: inner_clone.runtime_call,
            uniqueness: inner_clone.uniqueness,
            details: inner_clone.details,
            pub_key: pk,
            signature: sig2,
        });

        assert_eq!(assembled, direct);
        assert_eq!(
            Transaction::to_bytes(&assembled).unwrap(),
            Transaction::to_bytes(&direct).unwrap(),
        );
    }

    #[test]
    fn signed_to_bytes_roundtrips() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_bytes().unwrap();
        let sig: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let pk: [u8; 32] = keypair.public_key().try_into().unwrap();
        let signed = Transaction::from_parts(unsigned, sig, pk);

        let bytes = Transaction::to_bytes(&signed).unwrap();
        assert!(!bytes.is_empty());

        let deserialized: SignedTransaction =
            borsh::from_slice(&bytes).expect("should deserialize");
        assert_eq!(bytes, Transaction::to_bytes(&deserialized).unwrap());
    }

    #[test]
    fn to_base64_is_nonempty() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_bytes().unwrap();
        let sig: [u8; 64] = keypair.sign(&signable).try_into().unwrap();
        let pk: [u8; 32] = keypair.public_key().try_into().unwrap();
        let signed = Transaction::from_parts(unsigned, sig, pk);

        assert!(!Transaction::to_base64(&signed).unwrap().is_empty());
    }

    #[cfg(feature = "integration")]
    mod integration {
        use bullet_exchange_interface::message::PublicAction;

        use super::*;
        use crate::Network;

        #[tokio::test]
        async fn test_builder_build() {
            let network = std::env::var("BULLET_API_ENDPOINT")
                .map(|e| Network::from(e.as_str()))
                .unwrap_or(Network::Mainnet);

            let client =
                Client::builder().network(network).build().await.expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            let signed = Transaction::builder()
                .call_message(call_msg)
                .max_fee(10_000_000)
                .signer(&keypair)
                .client(&client)
                .build()
                .expect("Failed to build transaction");

            assert!(!Transaction::to_base64(&signed).unwrap().is_empty());
        }
    }
}
