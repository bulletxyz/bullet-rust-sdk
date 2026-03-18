//! Fluent transaction builder for constructing and submitting transactions.
//!
//! # Example
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
//! // Or use client defaults (keypair, max_fee, etc. set on client)
//! let response = Transaction::builder()
//!     .call_message(call_msg)
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
//! // Send later
//! client.send_transaction(&signed).await?;
//! ```

use bon::Builder;
use bullet_exchange_interface::transaction::{
    Amount, Gas, PriorityFeeBips, RuntimeCall, TxDetails, UniquenessData, Version0,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::SubmitTxResponse;
use crate::types::{CallMessage, SignedTransaction};
use crate::{Client, Keypair, SDKError, SDKResult};

/// A builder for constructing and submitting transactions.
///
/// Use `Transaction::builder()` to create a new builder, then chain
/// the required fields and call `.build(&client)` or `.send(&client)`.
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
/// - `signer` - Keypair to sign the transaction
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

impl<S: transaction_builder::State> TransactionBuilder<'_, S> {
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

        // Fall back to client defaults for optional fields
        let max_fee = tx.max_fee.unwrap_or_else(|| client.max_fee().0);
        let priority_fee_bips = tx
            .priority_fee_bips
            .unwrap_or_else(|| client.max_priority_fee_bips().0);
        let gas_limit = tx.gas_limit.or_else(|| client.gas_limit());

        // Get signer from builder or fall back to client default
        let signer = tx
            .signer
            .or_else(|| client.keypair())
            .ok_or(SDKError::MissingKeypair)?;

        build_signed_transaction(
            tx.call_message,
            max_fee,
            priority_fee_bips,
            gas_limit,
            signer,
            client,
        )
    }

    /// Sign and submit the transaction to the network.
    ///
    /// This is equivalent to calling `build()` followed by
    /// `client.send_transaction()`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No signer is provided and client has no default keypair
    /// - Signing fails
    /// - System time is unavailable
    /// - The network request fails
    pub async fn send(self, client: &Client) -> SDKResult<SubmitTxResponse>
    where
        S: transaction_builder::IsComplete,
    {
        let signed = self.build(client)?;
        client.submit_transaction(&signed).await
    }
}

/// Internal function to build a signed transaction.
fn build_signed_transaction(
    call_message: CallMessage,
    max_fee: u128,
    priority_fee_bips: u64,
    gas_limit: Option<Gas>,
    signer: &Keypair,
    client: &Client,
) -> SDKResult<SignedTransaction> {
    // Build unsigned transaction
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

    // Serialize and sign
    let unsigned = bullet_exchange_interface::transaction::UnsignedTransaction {
        runtime_call,
        uniqueness,
        details,
    };

    let mut data =
        borsh::to_vec(&unsigned).map_err(|e| SDKError::SerializationError(e.to_string()))?;
    data.extend_from_slice(client.chain_hash());

    let sig_bytes = signer.sign(&data);
    let signature: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|v: Vec<u8>| SDKError::InvalidSignatureLength(v.len()))?;

    let pk_bytes = signer.public_key();
    let pub_key: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|v: Vec<u8>| SDKError::InvalidPublicKeyLength(v.len()))?;

    Ok(SignedTransaction::V0(Version0 {
        runtime_call: unsigned.runtime_call,
        uniqueness: unsigned.uniqueness,
        details: unsigned.details,
        pub_key,
        signature,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time test: ensure the builder works correctly.
    #[allow(dead_code)]
    fn builder_compiles() {
        use bullet_exchange_interface::message::PublicAction;

        let keypair = Keypair::generate();
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        // This should compile: all required fields set
        let _builder = Transaction::builder()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .signer(&keypair);
    }

    // Compile-time test: optional priority_fee_bips can be set
    #[allow(dead_code)]
    fn optional_fields_work() {
        use bullet_exchange_interface::message::PublicAction;

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
        use crate::MAINNET_URL;
        use bullet_exchange_interface::message::PublicAction;

        #[tokio::test]
        async fn test_builder_build() {
            let endpoint = std::env::var("BULLET_API_ENDPOINT").unwrap_or(MAINNET_URL.to_string());

            let client = Client::builder()
                .url(&endpoint)
                .build()
                .await
                .expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            // Test build() - should succeed (just builds, doesn't validate on-chain)
            let signed = Transaction::builder()
                .call_message(call_msg)
                .max_fee(10_000_000)
                .signer(&keypair)
                .build(&client)
                .expect("Failed to build transaction");

            assert!(!Client::sign_to_base64(&signed).unwrap().is_empty());
        }
    }
}
