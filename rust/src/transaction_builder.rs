//! Fluent transaction builder with compile-time state validation.
//!
//! This module provides a type-safe builder pattern for constructing and
//! submitting transactions. The typestate pattern ensures at compile time
//! that all required fields are set before a transaction can be sent.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::{TransactionBuilder, Client, Keypair};
//!
//! let response = TransactionBuilder::new()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .signer(&keypair)
//!     .send(&client)
//!     .await?;
//! ```
//!
//! # Building Without Sending
//!
//! You can also build a signed transaction without sending it:
//!
//! ```ignore
//! let signed = TransactionBuilder::new()
//!     .call_message(call_msg)
//!     .max_fee(10_000_000)
//!     .signer(&keypair)
//!     .build(&client)?;
//!
//! // Send later
//! client.send_transaction(&signed).await?;
//! ```

use std::marker::PhantomData;

use bullet_exchange_interface::transaction::{
    Amount, PriorityFeeBips, RuntimeCall, TxDetails, UniquenessData, Version0,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::SubmitTxResponse;
use crate::types::{CallMessage, Transaction as SignedTransaction};
use crate::{Client, Keypair, SDKError, SDKResult};

// ============================================================================
// Typestate Markers
// ============================================================================

/// Builder state: waiting for call message to be set.
pub struct NeedsCallMessage;

/// Builder state: waiting for max fee to be set.
pub struct NeedsMaxFee;

/// Builder state: waiting for signer to be set.
pub struct NeedsSigner;

/// Builder state: all required fields set, ready to build or send.
pub struct Ready;

// ============================================================================
// TransactionBuilder
// ============================================================================

/// A fluent builder for constructing and submitting transactions.
///
/// Uses the typestate pattern to ensure all required fields are set at
/// compile time. The builder progresses through states:
///
/// `NeedsCallMessage` → `NeedsMaxFee` → `NeedsSigner` → `Ready`
///
/// Only in the `Ready` state can you call `build()` or `send()`.
pub struct TransactionBuilder<'a, State> {
    call_message: Option<CallMessage>,
    max_fee: Option<u128>,
    priority_fee_bips: u64,
    signer: Option<&'a Keypair>,
    _state: PhantomData<State>,
}

// ============================================================================
// Initial State: NeedsCallMessage
// ============================================================================

impl TransactionBuilder<'static, NeedsCallMessage> {
    /// Create a new transaction builder.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = TransactionBuilder::new()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .signer(&keypair);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        TransactionBuilder {
            call_message: None,
            max_fee: None,
            priority_fee_bips: 0,
            signer: None,
            _state: PhantomData,
        }
    }

    /// Set the call message for this transaction.
    ///
    /// This is the action to be executed (e.g., place order, withdraw, etc.).
    #[must_use]
    pub fn call_message(self, msg: CallMessage) -> TransactionBuilder<'static, NeedsMaxFee> {
        TransactionBuilder {
            call_message: Some(msg),
            max_fee: self.max_fee,
            priority_fee_bips: self.priority_fee_bips,
            signer: None,
            _state: PhantomData,
        }
    }
}

impl Default for TransactionBuilder<'static, NeedsCallMessage> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// State: NeedsMaxFee
// ============================================================================

impl TransactionBuilder<'static, NeedsMaxFee> {
    /// Set the maximum fee (in native units) willing to pay for this transaction.
    #[must_use]
    pub fn max_fee(self, fee: u128) -> TransactionBuilder<'static, NeedsSigner> {
        TransactionBuilder {
            call_message: self.call_message,
            max_fee: Some(fee),
            priority_fee_bips: self.priority_fee_bips,
            signer: None,
            _state: PhantomData,
        }
    }
}

// ============================================================================
// State: NeedsSigner
// ============================================================================

impl TransactionBuilder<'static, NeedsSigner> {
    /// Set the priority fee in basis points (optional, defaults to 0).
    ///
    /// Higher priority fees may result in faster transaction processing.
    #[must_use]
    pub fn priority_fee_bips(mut self, bips: u64) -> Self {
        self.priority_fee_bips = bips;
        self
    }

    /// Set the keypair used to sign this transaction.
    #[must_use]
    pub fn signer(self, keypair: &Keypair) -> TransactionBuilder<'_, Ready> {
        TransactionBuilder {
            call_message: self.call_message,
            max_fee: self.max_fee,
            priority_fee_bips: self.priority_fee_bips,
            signer: Some(keypair),
            _state: PhantomData,
        }
    }
}

// ============================================================================
// State: Ready
// ============================================================================

impl<'a> TransactionBuilder<'a, Ready> {
    /// Set the priority fee in basis points (optional, defaults to 0).
    ///
    /// Higher priority fees may result in faster transaction processing.
    #[must_use]
    pub fn priority_fee_bips(mut self, bips: u64) -> Self {
        self.priority_fee_bips = bips;
        self
    }

    /// Build the signed transaction without sending it.
    ///
    /// Use this if you want to inspect the transaction or send it later
    /// via `client.send_transaction(&signed)`.
    ///
    /// # Errors
    ///
    /// Returns an error if signing fails or system time is unavailable.
    pub fn build(self, client: &Client) -> SDKResult<SignedTransaction> {
        let call_message = self.call_message.expect("call_message guaranteed by typestate");
        let max_fee = self.max_fee.expect("max_fee guaranteed by typestate");
        let signer = self.signer.expect("signer guaranteed by typestate");

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
            gas_limit: None,
            max_priority_fee_bips: PriorityFeeBips(self.priority_fee_bips),
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

    /// Sign and submit the transaction to the network.
    ///
    /// This is equivalent to calling `build()` followed by
    /// `client.send_transaction()`.
    ///
    /// # Errors
    ///
    /// Returns an error if signing fails, system time is unavailable,
    /// or the network request fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = TransactionBuilder::new()
    ///     .call_message(call_msg)
    ///     .max_fee(10_000_000)
    ///     .signer(&keypair)
    ///     .send(&client)
    ///     .await?;
    /// ```
    pub async fn send(self, client: &Client) -> SDKResult<SubmitTxResponse> {
        let signed = self.build(client)?;
        client.submit_transaction(&signed).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time test: ensure the typestate transitions work correctly.
    // This test doesn't run, it just needs to compile.
    #[allow(dead_code)]
    fn typestate_compiles() {
        use bullet_exchange_interface::message::PublicAction;

        let keypair = Keypair::generate();
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        // This should compile: correct order of builder methods
        let _builder: TransactionBuilder<'_, Ready> = TransactionBuilder::new()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .priority_fee_bips(100)
            .signer(&keypair);
    }

    // Compile-time test: optional methods can be called in Ready state too
    #[allow(dead_code)]
    fn optional_methods_in_ready_state() {
        use bullet_exchange_interface::message::PublicAction;

        let keypair = Keypair::generate();
        let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

        let _builder: TransactionBuilder<'_, Ready> = TransactionBuilder::new()
            .call_message(call_msg)
            .max_fee(10_000_000)
            .signer(&keypair)
            .priority_fee_bips(100); // Can set after signer
    }

    #[cfg(feature = "integration")]
    mod integration {
        use super::*;
        use bullet_exchange_interface::message::PublicAction;
        use crate::MAINNET_URL;

        #[tokio::test]
        async fn test_builder_send() {
            let endpoint = std::env::var("BULLET_API_ENDPOINT").unwrap_or(MAINNET_URL.to_string());

            let client = Client::new(&endpoint, None)
                .await
                .expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg = CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            // Test build() - should succeed (just builds, doesn't validate on-chain)
            let signed = TransactionBuilder::new()
                .call_message(call_msg.clone())
                .max_fee(10_000_000)
                .signer(&keypair)
                .build(&client)
                .expect("Failed to build transaction");

            assert!(!Client::sign_to_base64(&signed).unwrap().is_empty());
        }
    }
}
