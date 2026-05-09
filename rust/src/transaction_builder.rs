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

use std::sync::OnceLock;

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
use sov_universal_wallet::schema::{Link, Primitive};
use sov_universal_wallet::ty::{IntegerType, Ty};
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

    /// Serialize into the human-readable JSON bytes for offchain signing.
    ///
    /// This is the message external Solana wallets should sign when the backend
    /// uses the `solanaSimple` authenticator. The resulting signature must be
    /// wrapped with [`SolanaOffchainTransaction::from_parts`] and submitted via
    /// [`Client::send_offchain_transaction`].
    pub fn to_message_bytes(&self) -> SDKResult<Vec<u8>> {
        let mut message = serde_json::to_value(&self.inner)?;
        stringify_offchain_integer_newtypes(&mut message)?;

        let Value::Object(mut message) = message else {
            return Err(SDKError::SerializationError(
                "unsigned transaction serialized to non-object JSON".to_string(),
            ));
        };
        message.insert("chain_name".to_string(), Value::String(self.chain_name.clone()));
        message.insert("chain_hash".to_string(), Value::String(chain_hash_hex(&self.chain_hash)));
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
        client: &Client,
    ) -> SDKResult<UnsignedTransaction> {
        // Check whether the call message was part of the schema validation.
        if !Client::call_message_was_validated(&call_message, client.user_actions().as_deref()) {
            return Err(SDKError::UnsupportedCallMessage(call_message.msg_type()));
        }

        let runtime_call = RuntimeCall::Exchange(call_message);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SDKError::SystemTimeError)?
            .as_millis() as u64;
        let uniqueness = UniquenessData::Generation(timestamp);
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
        Ok(Self { signed_message: tx.to_message_bytes()?, pubkey, signature })
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
        let body = signed.to_base64()?;
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
        Ok(SDKError::ApiError(error))
    }
}

fn chain_hash_hex(chain_hash: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(chain_hash))
}

#[derive(Clone, Debug)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
    AnyArrayItem,
    AnyObjectValue,
}

type JsonPath = Vec<JsonPathSegment>;

static OFFCHAIN_INTEGER_NEWTYPE_PATHS: OnceLock<Result<Vec<JsonPath>, String>> = OnceLock::new();

fn stringify_offchain_integer_newtypes(value: &mut Value) -> SDKResult<()> {
    for path in offchain_integer_newtype_paths()? {
        stringify_json_path(value, path);
    }
    Ok(())
}

fn offchain_integer_newtype_paths() -> SDKResult<&'static [JsonPath]> {
    OFFCHAIN_INTEGER_NEWTYPE_PATHS
        .get_or_init(build_offchain_integer_newtype_paths)
        .as_deref()
        .map_err(|e| SDKError::SerializationError(e.clone()))
}

fn build_offchain_integer_newtype_paths() -> Result<Vec<JsonPath>, String> {
    let schema = Schema::of_single_type::<RawUnsignedTransaction>().map_err(|e| e.to_string())?;
    let mut paths = Vec::new();
    collect_integer_newtype_paths(&schema, &Link::ByIndex(0), false, &mut Vec::new(), &mut paths)?;
    Ok(paths)
}

fn collect_integer_newtype_paths(
    schema: &Schema,
    link: &Link,
    allow_newtype_stringify: bool,
    path: &mut JsonPath,
    paths: &mut Vec<JsonPath>,
) -> Result<(), String> {
    let Link::ByIndex(index) = link else {
        return Ok(());
    };
    let schema_type = schema.types().get(*index).ok_or_else(|| {
        format!("schema index {index} not found while serializing offchain message")
    })?;

    match schema_type {
        Ty::Tuple(tuple)
            if allow_newtype_stringify
                && tuple.fields.len() == 1
                && link_contains_wide_integer(schema, &tuple.fields[0].value)? =>
        {
            paths.push(path.clone());
        }
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => {
            collect_integer_newtype_paths(
                schema,
                &tuple.fields[0].value,
                allow_newtype_stringify,
                path,
                paths,
            )?;
        }
        Ty::Tuple(tuple) => {
            for (field_index, field) in tuple.fields.iter().enumerate() {
                path.push(JsonPathSegment::Index(field_index));
                collect_integer_newtype_paths(
                    schema,
                    &field.value,
                    allow_newtype_stringify,
                    path,
                    paths,
                )?;
                path.pop();
            }
        }
        Ty::Struct(data) => {
            let serde_metadata = schema.serde_metadata().get(*index);
            for (field_index, field) in data.fields.iter().enumerate() {
                let key = serde_metadata
                    .and_then(|metadata| metadata.fields_or_variants.get(field_index))
                    .map(|field| field.name.as_str())
                    .unwrap_or(field.display_name.as_str());
                path.push(JsonPathSegment::Key(key.to_string()));
                collect_integer_newtype_paths(schema, &field.value, true, path, paths)?;
                path.pop();
            }
        }
        Ty::Enum(data) => {
            let serde_metadata = schema.serde_metadata().get(*index);
            for (variant_index, variant) in data.variants.iter().enumerate() {
                let Some(link) = &variant.value else {
                    continue;
                };
                let key = serde_metadata
                    .and_then(|metadata| metadata.fields_or_variants.get(variant_index))
                    .map(|variant| variant.name.as_str())
                    .unwrap_or(variant.name.as_str());
                path.push(JsonPathSegment::Key(key.to_string()));
                collect_integer_newtype_paths(schema, link, false, path, paths)?;
                path.pop();
            }
        }
        Ty::Option { value: inner } => {
            collect_integer_newtype_paths(schema, inner, allow_newtype_stringify, path, paths)?;
        }
        Ty::Array { value: inner, .. } | Ty::Vec { value: inner } => {
            path.push(JsonPathSegment::AnyArrayItem);
            collect_integer_newtype_paths(schema, inner, allow_newtype_stringify, path, paths)?;
            path.pop();
        }
        Ty::Map { value: inner, .. } => {
            path.push(JsonPathSegment::AnyObjectValue);
            collect_integer_newtype_paths(schema, inner, allow_newtype_stringify, path, paths)?;
            path.pop();
        }
        _ => {}
    }
    Ok(())
}

fn link_contains_wide_integer(schema: &Schema, link: &Link) -> Result<bool, String> {
    match link {
        Link::Immediate(Primitive::Integer(kind, _)) => Ok(is_wide_integer(*kind)),
        Link::Immediate(_) => Ok(false),
        Link::ByIndex(index) => {
            let schema_type = schema.types().get(*index).ok_or_else(|| {
                format!("schema index {index} not found while serializing offchain message")
            })?;
            match schema_type {
                Ty::Integer(kind, _) => Ok(is_wide_integer(*kind)),
                Ty::Tuple(tuple) if tuple.fields.len() == 1 => {
                    link_contains_wide_integer(schema, &tuple.fields[0].value)
                }
                Ty::Option { value } | Ty::Array { value, .. } | Ty::Vec { value } => {
                    link_contains_wide_integer(schema, value)
                }
                _ => Ok(false),
            }
        }
        Link::Placeholder | Link::IndexedPlaceholder(_) => {
            Err("unresolved schema link while serializing offchain message".to_string())
        }
    }
}

fn is_wide_integer(kind: IntegerType) -> bool {
    matches!(kind, IntegerType::u64 | IntegerType::u128 | IntegerType::i64 | IntegerType::i128)
}

fn stringify_json_path(value: &mut Value, path: &[JsonPathSegment]) {
    let Some((segment, rest)) = path.split_first() else {
        stringify_json_number(value);
        return;
    };

    match segment {
        JsonPathSegment::Key(key) => {
            if let Some(value) = value.as_object_mut().and_then(|object| object.get_mut(key)) {
                stringify_json_path(value, rest);
            }
        }
        JsonPathSegment::Index(index) => {
            if let Some(value) = value.as_array_mut().and_then(|values| values.get_mut(*index)) {
                stringify_json_path(value, rest);
            }
        }
        JsonPathSegment::AnyArrayItem => {
            if let Some(values) = value.as_array_mut() {
                for value in values {
                    stringify_json_path(value, rest);
                }
            }
        }
        JsonPathSegment::AnyObjectValue => {
            if let Some(object) = value.as_object_mut() {
                for value in object.values_mut() {
                    stringify_json_path(value, rest);
                }
            }
        }
    }
}

fn stringify_json_number(value: &mut Value) {
    match value {
        Value::Number(number) => {
            *value = Value::String(number.to_string());
        }
        Value::Array(values) => {
            for value in values {
                stringify_json_number(value);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                stringify_json_number(value);
            }
        }
        _ => {}
    }
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
                "chain_hash": chain_hash_hex(&chain_hash),
            })
            .to_string()
            .into_bytes(),
            pubkey: [8u8; 32],
            signature: [9u8; 64],
        }
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
    fn to_message_bytes_includes_chain_hash_domain_separator() {
        let unsigned = test_unsigned_tx();

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["chain_hash"], format!("0x{}", hex::encode([42u8; 32])));
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
    fn to_message_bytes_serializes_integer_newtypes_by_schema_type() {
        let mut unsigned = test_unsigned_tx();
        unsigned.inner.details.max_priority_fee_bips = PriorityFeeBips(u64::MAX);

        let bytes = unsigned.to_message_bytes().unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["details"]["max_priority_fee_bips"], u64::MAX.to_string());
        assert_eq!(message["details"]["chain_id"], 1);
        assert_eq!(message["uniqueness"]["generation"], 12345);
    }

    #[test]
    fn to_message_bytes_serializes_order_ids_as_strings() {
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
            "10000000000"
        );
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
        assert_eq!(tx.pubkey, pub_key);
        assert_eq!(tx.signature, signature);
        assert!(!tx.to_base64().unwrap().is_empty());

        let roundtrip: SolanaOffchainTransaction =
            borsh::from_slice(&tx.to_bytes().unwrap()).expect("should deserialize");
        assert_eq!(roundtrip, tx);
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
