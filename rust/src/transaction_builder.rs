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
use bullet_exchange_interface::transaction::{
    Amount, Gas, PriorityFeeBips, RuntimeCall, Transaction as SignedTransaction, TxDetails,
    UniquenessData, UnsignedTransaction as RawUnsignedTransaction, Version0,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::codegen::Error::ErrorResponse;
use crate::generated::types::{SubmitTxRequest, SubmitTxResponse};
use crate::types::CallMessage;
use crate::{ApiErrorResponse, Client, Keypair, SDKError, SDKResult};

// ── UnsignedTransaction ──────────────────────────────────────────────────────

/// An unsigned transaction with the chain hash baked in.
///
/// Created by [`UnsignedTransaction::build`]. Contains everything
/// needed to produce signable bytes without a client reference.
pub struct UnsignedTransaction {
    inner: RawUnsignedTransaction,
    chain_hash: [u8; 32],
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
        // Check whether the call-message was part of the schema validation
        if let Some(user_actions) = client.user_actions()
            && let CallMessage::User(ref call) = call_message
            && !user_actions.contains(&call.into())
        {
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
            inner: RawUnsignedTransaction {
                runtime_call,
                uniqueness,
                details,
            },
            chain_hash: client.chain_hash(),
        })
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
        let signer = signer
            .or_else(|| client.keypair())
            .ok_or(SDKError::MissingKeypair)?;

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
        let RawUnsignedTransaction {
            runtime_call,
            uniqueness,
            details,
        } = tx.inner;
        SignedTransaction::V0(Version0 {
            runtime_call,
            uniqueness,
            details,
            pub_key,
            signature,
        })
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
                if inner.message.contains("Invalid signature") {
                    self.update_schema().await?;
                    // map the error to 429 to trigger the retry path
                    return Err(SDKError::ApiError(ApiErrorResponse {
                        status: 429,
                        details: None,
                        message: "Update Schema".to_string(),
                    }));
                }
                Err(SDKError::ApiError(inner))
            }
            Ok(r) => Ok(r.into_inner()),
            Err(e) => Err(e.into()),
        }
    }
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
                .client(&client)
                .build()
                .expect("Failed to build transaction");

            assert!(!Transaction::to_base64(&signed).unwrap().is_empty());
        }
    }
}
