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
//! let signable = unsigned.to_bytes()?;
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
    UniquenessData, UnsignedTransaction as RawUnsignedTransaction, Version0,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::{SubmitTxRequest, SubmitTxResponse};
use crate::types::CallMessage;
use crate::{Client, Keypair, SDKError, SDKResult};

// ── UnsignedTransaction ──────────────────────────────────────────────────────

/// An unsigned transaction with the chain hash baked in.
///
/// Created by [`TransactionBuilder::build_unsigned`]. Contains everything
/// needed to produce signable bytes without a client reference.
pub struct UnsignedTransaction {
    inner: RawUnsignedTransaction,
    chain_hash: [u8; 32],
}

impl UnsignedTransaction {
    /// Serialize into the bytes that must be signed.
    ///
    /// Borsh-serializes the transaction and appends the chain hash (32 bytes)
    /// as a domain separator.
    pub fn to_bytes(&self) -> SDKResult<Vec<u8>> {
        let mut data = borsh::to_vec(&self.inner)
            .map_err(|e| SDKError::SerializationError(e.to_string()))?;
        data.extend_from_slice(&self.chain_hash);
        Ok(data)
    }
}

// ── Transaction builder ──────────────────────────────────────────────────────

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
    /// Use after signing the bytes from [`UnsignedTransaction::to_bytes`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .build_unsigned(&client)?;
    ///
    /// let signable = unsigned.to_bytes()?;
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

        let RawUnsignedTransaction {
            runtime_call,
            uniqueness,
            details,
        } = tx.inner;
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
    /// The returned [`UnsignedTransaction`] contains the chain hash, so
    /// [`to_bytes()`](UnsignedTransaction::to_bytes) produces signable bytes
    /// without needing a client reference.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = Transaction::builder()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .build_unsigned(&client)?;
    ///
    /// let signable = unsigned.to_bytes()?;
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
        let inner = make_unsigned(tx.call_message, max_fee, priority_fee_bips, gas_limit, client)?;
        Ok(UnsignedTransaction {
            inner,
            chain_hash: *client.chain_hash(),
        })
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

        let inner = make_unsigned(
            tx.call_message,
            max_fee,
            priority_fee_bips,
            gas_limit,
            client,
        )?;
        let unsigned = UnsignedTransaction {
            inner,
            chain_hash: *client.chain_hash(),
        };
        let data = unsigned.to_bytes()?;
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

/// Build a raw unsigned transaction from resolved parameters.
fn make_unsigned(
    call_message: CallMessage,
    max_fee: u128,
    priority_fee_bips: u64,
    gas_limit: Option<Gas>,
    client: &Client,
) -> SDKResult<RawUnsignedTransaction> {
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

    Ok(RawUnsignedTransaction {
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
        let inner = RawUnsignedTransaction {
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
        };
        UnsignedTransaction {
            inner,
            chain_hash: [42u8; 32],
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
    fn from_parts_matches_direct_construction() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        // Via from_parts
        let signable = unsigned.to_bytes().unwrap();
        let sig = keypair.sign(&signable);
        let pk = keypair.public_key();

        // Reconstruct for direct comparison (from_parts consumes unsigned)
        let chain_hash = unsigned.chain_hash;
        let inner_clone = RawUnsignedTransaction {
            runtime_call: unsigned.inner.runtime_call.clone(),
            uniqueness: unsigned.inner.uniqueness.clone(),
            details: unsigned.inner.details.clone(),
        };
        let assembled = Transaction::from_parts(unsigned, &sig, &pk).unwrap();

        // Direct Version0 construction
        let mut data = borsh::to_vec(&inner_clone).unwrap();
        data.extend_from_slice(&chain_hash);
        let sig2 = keypair.sign(&data);
        let direct = SignedTransaction::V0(Version0 {
            runtime_call: inner_clone.runtime_call,
            uniqueness: inner_clone.uniqueness,
            details: inner_clone.details,
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
    fn signed_to_bytes_roundtrips() {
        let keypair = Keypair::generate();
        let unsigned = test_unsigned_tx();

        let signable = unsigned.to_bytes().unwrap();
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

        let signable = unsigned.to_bytes().unwrap();
        let signed =
            Transaction::from_parts(unsigned, &keypair.sign(&signable), &keypair.public_key())
                .unwrap();

        assert!(!Transaction::to_base64(&signed).unwrap().is_empty());
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

    #[cfg(feature = "integration")]
    mod integration {
        use super::*;
        use crate::Network;
        use bullet_exchange_interface::message::PublicAction;

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
