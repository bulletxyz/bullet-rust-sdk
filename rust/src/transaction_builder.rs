//! Fluent transaction builder for constructing and submitting transactions.
//!
//! All transaction construction goes through the builder pattern:
//!
//! ```ignore
//! use bullet_rust_sdk::{Transaction, Client, Keypair};
//!
//! // Build and send with explicit signer
//! let response = Transaction::builder()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .signer(&keypair)
//!     .send(&client)
//!     .await?;
//!
//! // Or just build without sending
//! let signed = Transaction::builder()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .signer(&keypair)
//!     .build(&client)?;
//!
//! // External signing
//! let unsigned = Transaction::builder()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .build_unsigned(&client)?;
//!
//! let signable = client.to_signable_bytes(&unsigned)?;
//! let signature = external_signer.sign(&signable);
//! let signed = Transaction::from_parts(unsigned, &signature, &pub_key)?;
//!
//! // Submit later
//! client.send_transaction(&signed).await?;
//! ```

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bon::Builder;
use bullet_exchange_interface::transaction::{
    Amount, Gas, PriorityFeeBips, RuntimeCall, Transaction as SignedTransaction, TxDetails,
    UniquenessData, UnsignedTransaction, Version0,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::{SubmitTxRequest, SubmitTxResponse};
use crate::types::CallMessage;
use crate::{Client, Keypair, SDKError, SDKResult};

/// A builder for constructing and submitting transactions.
///
/// Use `Transaction::builder()` to create a new builder, then chain
/// the required fields and call `.build(&client)`, `.build_unsigned(&client)`,
/// or `.send(&client)`.
///
/// # Required Fields
///
/// - `call_message` - The action to execute (e.g., place order, withdraw)
///
/// # Optional Fields (fall back to client defaults if not set)
///
/// - `max_fee` - Maximum fee willing to pay (in base units)
/// - `priority_fee_bips` - Priority fee in basis points
/// - `gas_limit` - Optional gas limit
/// - `signer` - Keypair to sign the transaction (not required for `build_unsigned`)
///
/// # Example
///
/// ```ignore
/// // With explicit values
/// let response = Transaction::builder()
///     .call_message(call_msg)
///     .max_fee(10_000_000)
///     .priority_fee_bips(100)
///     .signer(&keypair)
///     .send(&client)
///     .await?;
///
/// // Using client defaults
/// let response = Transaction::builder()
///     .call_message(call_msg)
///     .send(&client)
///     .await?;
/// ```
#[derive(Builder)]
#[builder(start_fn = builder, finish_fn = __build)]
pub struct Transaction<'a> {
    /// The action to be executed (e.g., place order, withdraw, etc.).
    call_message: CallMessage,

    /// Maximum fee (in base units) willing to pay for this transaction.
    /// Falls back to client default if not set.
    max_fee: Option<u128>,

    /// Priority fee in basis points. Higher values may result in faster processing.
    /// Falls back to client default if not set.
    priority_fee_bips: Option<u64>,

    /// Optional gas limit. Falls back to client default if not set.
    gas_limit: Option<Gas>,

    /// Keypair used to sign this transaction.
    /// Falls back to client default if not set.
    signer: Option<&'a Keypair>,
}

// ── Transaction associated functions ─────────────────────────────────────────

impl Transaction<'_> {
    /// Assemble a signed transaction from an unsigned transaction, a 64-byte
    /// Ed25519 signature, and a 32-byte public key.
    ///
    /// Use after signing the bytes from [`Client::to_signable_bytes`] with an
    /// external signer.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .build_unsigned(&client)?;
    ///
    /// let signable = client.to_signable_bytes(&unsigned)?;
    /// let signature = external_signer.sign(&signable);
    /// let signed = Transaction::from_parts(unsigned, &signature, &pub_key)?;
    /// ```
    pub fn from_parts(
        tx: UnsignedTransaction,
        signature: &[u8],
        pub_key: &[u8],
    ) -> SDKResult<SignedTransaction> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| SDKError::InvalidSignatureLength(signature.len()))?;
        let pub_key: [u8; 32] = pub_key
            .try_into()
            .map_err(|_| SDKError::InvalidPublicKeyLength(pub_key.len()))?;

        let UnsignedTransaction {
            runtime_call,
            uniqueness,
            details,
        } = tx;
        Ok(SignedTransaction::V0(Version0 {
            runtime_call,
            uniqueness,
            details,
            pub_key,
            signature,
        }))
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

// ── Builder methods ──────────────────────────────────────────────────────────

impl<S: transaction_builder::State> TransactionBuilder<'_, S> {
    /// Build the unsigned transaction without signing it.
    ///
    /// Returns an `UnsignedTransaction` that can be signed externally via
    /// [`Client::to_signable_bytes`] and [`Transaction::from_parts`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .build_unsigned(&client)?;
    ///
    /// let signable = client.to_signable_bytes(&unsigned)?;
    /// let signature = external_signer.sign(&signable);
    /// let signed = Transaction::from_parts(unsigned, &signature, &pub_key)?;
    /// ```
    pub fn build_unsigned(self, client: &Client) -> SDKResult<UnsignedTransaction>
    where
        S: transaction_builder::IsComplete,
    {
        let tx = self.__build();
        let max_fee = tx.max_fee.unwrap_or_else(|| client.max_fee().0);
        let priority_fee_bips = tx
            .priority_fee_bips
            .unwrap_or_else(|| client.max_priority_fee_bips().0);
        let gas_limit = tx.gas_limit.or_else(|| client.gas_limit());
        make_unsigned(tx.call_message, max_fee, priority_fee_bips, gas_limit, client)
    }

    /// Build the signed transaction without sending it.
    ///
    /// Use this if you want to inspect the transaction or send it later
    /// via `client.send_transaction(&signed)`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No signer is provided and client has no default keypair
    /// - Signing fails
    /// - System time is unavailable
    pub fn build(self, client: &Client) -> SDKResult<SignedTransaction>
    where
        S: transaction_builder::IsComplete,
    {
        let tx = self.__build();

        let max_fee = tx.max_fee.unwrap_or_else(|| client.max_fee().0);
        let priority_fee_bips = tx
            .priority_fee_bips
            .unwrap_or_else(|| client.max_priority_fee_bips().0);
        let gas_limit = tx.gas_limit.or_else(|| client.gas_limit());

        let signer = tx
            .signer
            .or_else(|| client.keypair())
            .ok_or(SDKError::MissingKeypair)?;

        let unsigned = make_unsigned(
            tx.call_message,
            max_fee,
            priority_fee_bips,
            gas_limit,
            client,
        )?;
        let data = client.to_signable_bytes(&unsigned)?;
        let sig_bytes = signer.sign(&data);
        let pk_bytes = signer.public_key();
        Transaction::from_parts(unsigned, &sig_bytes, &pk_bytes)
    }

    /// Sign and submit the transaction to the network.
    ///
    /// This is equivalent to calling `build()` followed by
    /// `client.send_transaction()`.
    pub async fn send(self, client: &Client) -> SDKResult<SubmitTxResponse>
    where
        S: transaction_builder::IsComplete,
    {
        let signed = self.build(client)?;
        client.send_transaction(&signed).await
    }
}

// ── Client methods ───────────────────────────────────────────────────────────

impl Client {
    /// Serialize an unsigned transaction into the bytes that need to be signed.
    ///
    /// Borsh-serializes the transaction and appends this client's chain hash
    /// (32 bytes) as a domain separator.
    pub fn to_signable_bytes(&self, tx: &UnsignedTransaction) -> SDKResult<Vec<u8>> {
        let mut data =
            borsh::to_vec(tx).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        data.extend_from_slice(self.chain_hash());
        Ok(data)
    }

    /// Send a signed transaction to the network.
    ///
    /// Returns the response from the sequencer.
    pub async fn send_transaction(
        &self,
        signed: &SignedTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        let body = Transaction::to_base64(signed)?;
        let response = self.client().submit_tx(&SubmitTxRequest { body }).await?;
        Ok(response.into_inner())
    }
}

// ── Internal ─────────────────────────────────────────────────────────────────

/// Build an unsigned transaction from resolved parameters.
fn make_unsigned(
    call_message: CallMessage,
    max_fee: u128,
    priority_fee_bips: u64,
    gas_limit: Option<Gas>,
    client: &Client,
) -> SDKResult<UnsignedTransaction> {
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
        runtime_call,
        uniqueness,
        details,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bullet_exchange_interface::message::PublicAction;
    use bullet_exchange_interface::transaction::{
        Amount, PriorityFeeBips, RuntimeCall, TxDetails, UniquenessData,
    };

    fn test_unsigned_tx() -> UnsignedTransaction {
        UnsignedTransaction {
            runtime_call: RuntimeCall::Exchange(CallMessage::Public(
                PublicAction::ApplyFunding { addresses: vec![] },
            )),
            uniqueness: UniquenessData::Generation(12345),
            details: TxDetails {
                chain_id: 1,
                max_fee: Amount(10_000_000),
                gas_limit: None,
                max_priority_fee_bips: PriorityFeeBips(0),
            },
        }
    }

    #[test]
    fn from_parts_matches_direct_construction() {
        let chain_hash = [42u8; 32];
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        // Via from_parts
        let mut signable = borsh::to_vec(&unsigned).unwrap();
        signable.extend_from_slice(&chain_hash);
        let sig = keypair.sign(&signable);
        let pk = keypair.public_key();
        let assembled =
            Transaction::from_parts(unsigned.clone(), &sig, &pk).unwrap();

        // Direct Version0 construction
        let mut data = borsh::to_vec(&unsigned).unwrap();
        data.extend_from_slice(&chain_hash);
        let sig2 = keypair.sign(&data);
        let direct = SignedTransaction::V0(Version0 {
            runtime_call: unsigned.runtime_call,
            uniqueness: unsigned.uniqueness,
            details: unsigned.details,
            pub_key: pk.clone().try_into().unwrap(),
            signature: sig2.try_into().unwrap(),
        });

        assert_eq!(assembled, direct);
        assert_eq!(
            Transaction::to_bytes(&assembled).unwrap(),
            Transaction::to_bytes(&direct).unwrap(),
        );
    }

    #[test]
    fn to_bytes_roundtrips() {
        let chain_hash = [0u8; 32];
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let mut signable = borsh::to_vec(&unsigned).unwrap();
        signable.extend_from_slice(&chain_hash);
        let signed =
            Transaction::from_parts(unsigned, &keypair.sign(&signable), &keypair.public_key())
                .unwrap();

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

        let mut signable = borsh::to_vec(&unsigned).unwrap();
        signable.extend_from_slice(&[0u8; 32]);
        let signed =
            Transaction::from_parts(unsigned, &keypair.sign(&signable), &keypair.public_key())
                .unwrap();

        let b64 = Transaction::to_base64(&signed).unwrap();
        assert!(!b64.is_empty());
    }

    #[test]
    fn from_parts_rejects_invalid_signature_length() {
        let unsigned = test_unsigned_tx();
        let result = Transaction::from_parts(unsigned, &[0u8; 63], &[0u8; 32]);
        assert!(matches!(result, Err(SDKError::InvalidSignatureLength(63))));
    }

    #[test]
    fn from_parts_rejects_invalid_pubkey_length() {
        let unsigned = test_unsigned_tx();
        let result = Transaction::from_parts(unsigned, &[0u8; 64], &[0u8; 31]);
        assert!(matches!(
            result,
            Err(SDKError::InvalidPublicKeyLength(31))
        ));
    }

    // Compile-time test: ensure the builder works correctly.
    #[allow(dead_code)]
    fn builder_compiles() {
        let keypair = Keypair::generate();
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let _builder = Transaction::builder()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .signer(&keypair);
    }

    // Compile-time test: optional priority_fee_bips can be set
    #[allow(dead_code)]
    fn optional_fields_work() {
        let keypair = Keypair::generate();
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let _builder = Transaction::builder()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .priority_fee_bips(100)
            .signer(&keypair);
    }

    #[cfg(feature = "integration")]
    mod integration {
        use super::*;
        use crate::Network;

        #[tokio::test]
        async fn test_builder_build() {
            let network = std::env::var("BULLET_API_ENDPOINT")
                .map(|e| Network::from(e.as_str()))
                .unwrap_or(Network::Mainnet);

            let client = Client::builder()
                .network(network)
                .build()
                .await
                .expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            let signed = Transaction::builder()
                .call_message(call_msg)
                .max_fee(10_000_000)
                .signer(&keypair)
                .build(&client)
                .expect("Failed to build transaction");

            assert!(!Transaction::to_base64(&signed).unwrap().is_empty());
        }
    }
}
