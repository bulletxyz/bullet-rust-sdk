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
use web_time::{SystemTime, UNIX_EPOCH};

use crate::codegen::Error::ErrorResponse;
use crate::generated::types::{ApiErrorResponse, SubmitTxRequest, SubmitTxResponse};
use crate::types::CallMessage;
use crate::{Client, Keypair, SDKError, SDKResult};

// ── UnsignedTransaction ──────────────────────────────────────────────────────

/// An unsigned transaction with the chain hash baked in.
///
/// Created by [`UnsignedTransaction::build`]. Contains everything
/// needed to produce signable bytes without a client reference.
pub struct UnsignedTransaction {
    inner: RawUnsignedTransaction,
    chain_hash: [u8; 32],
    chain_name: String,
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
        let preamble = make_solana_preamble(pubkey, &self.chain_hash, message_len);
        let mut result = Vec::with_capacity(preamble.len() + json_bytes.len());
        result.extend_from_slice(&preamble);
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
        /// Uniqueness generation value. Defaults to the current unix timestamp
        /// in milliseconds, giving a ~5-second deduplication window with the
        /// sequencer's default 5000-generation window.
        generation: Option<u64>,
        client: &Client,
    ) -> SDKResult<UnsignedTransaction> {
        // Check whether the call message was part of the schema validation.
        if !Client::call_message_was_validated(&call_message, client.user_actions().as_deref()) {
            return Err(SDKError::UnsupportedCallMessage(call_message.msg_type()));
        }

        let runtime_call = RuntimeCall::Exchange(call_message);
        let generation = match generation {
            Some(g) => g,
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| SDKError::SystemTimeError)?
                .as_millis() as u64,
        };
        let uniqueness = UniquenessData::Generation(generation);
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

/// Build the 85-byte Solana off-chain message preamble.
///
/// Layout (85 bytes):
///   [0..16]  signing domain (0xff + "solana offchain")
///   [16]     header_version = 0
///   [17..49] application_domain = chain_hash (32 bytes)
///   [49]     message_format = 0
///   [50]     signer_count = 1
///   [51..83] pubkey (32 bytes)
///   [83..85] message_length as LE u16
fn make_solana_preamble(pubkey: &[u8; 32], chain_hash: &[u8; 32], message_length: u16) -> [u8; 85] {
    let mut preamble = [0u8; 85];
    preamble[0..16].copy_from_slice(&SOLANA_SIGNING_DOMAIN);
    // header_version = 0 (already 0)
    preamble[17..49].copy_from_slice(chain_hash);
    // message_format = 0 (already 0)
    preamble[50] = 1; // signer_count
    preamble[51..83].copy_from_slice(pubkey);
    preamble[83..85].copy_from_slice(&message_length.to_le_bytes());
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
        generation: Option<u64>,
        signer: Option<&Keypair>,
        client: &Client,
    ) -> SDKResult<SignedTransaction> {
        let signer = signer.or_else(|| client.keypair()).ok_or(SDKError::MissingKeypair)?;

        let max_fee = max_fee.unwrap_or_else(|| client.max_fee().0);
        let priority_fee_bips =
            priority_fee_bips.unwrap_or_else(|| client.max_priority_fee_bips().0);
        let gas_limit = gas_limit.or_else(|| client.gas_limit());

        let unsigned = UnsignedTransaction::builder()
            .call_message(call_message)
            .max_fee(max_fee)
            .priority_fee_bips(priority_fee_bips)
            .maybe_gas_limit(gas_limit)
            .maybe_generation(generation)
            .client(client)
            .build()?;

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

    /// Send a Solana offchain transaction to the sequencer.
    ///
    /// This posts to `/sequencer/solana_offchain_txs`, which accepts the
    /// JSON-based Solana authenticator payload used by external Solana wallets.
    pub async fn send_offchain_transaction(
        &self,
        signed: &SolanaOffchainTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        self.post_to_solana_offchain_url(signed.to_base64()?).await
    }

    /// Send a Solana Ledger transaction to the sequencer.
    ///
    /// Posts the spec-compliant wire format to `/sequencer/solana_offchain_txs`.
    /// Use after signing with [`UnsignedTransaction::to_ledger_signable_bytes`]
    /// and assembling via [`SolanaLedgerTransaction::from_parts`].
    pub async fn send_ledger_transaction(
        &self,
        signed: &SolanaLedgerTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        self.post_to_solana_offchain_url(signed.to_base64()).await
    }

    async fn post_to_solana_offchain_url(&self, body: String) -> SDKResult<SubmitTxResponse> {
        let response = self
            .http_client
            .post(self.solana_offchain_url())
            .json(&SubmitTxRequest { body })
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;

        if status.is_success() {
            return serde_json::from_slice::<SubmitTxResponse>(&bytes).map_err(Into::into);
        }

        let error = serde_json::from_slice::<ApiErrorResponse>(&bytes).unwrap_or_else(|_| {
            ApiErrorResponse {
                status: status.as_u16(),
                message: String::from_utf8_lossy(&bytes).into_owned(),
                details: None,
                error_id: None,
            }
        });
        Err(self.submit_tx_api_error(error).await?)
    }

    async fn submit_tx_api_error(&self, error: ApiErrorResponse) -> SDKResult<SDKError> {
        if error.status == 401 && error.message.contains("Invalid signature") {
            self.update_schema().await?;
            // The transaction was signed against an old chain hash and must be re-signed.
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
            .and(path("/sequencer/solana_offchain_txs"))
            .respond_with(offchain_response)
            .mount(&server)
            .await;

        let client = Client::builder()
            .network(server.uri())
            .solana_offchain_url(format!("{}/sequencer/solana_offchain_txs", server.uri()))
            .build()
            .await
            .unwrap();

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

        let preamble = make_solana_preamble(&pubkey, &chain_hash, message_length);

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
