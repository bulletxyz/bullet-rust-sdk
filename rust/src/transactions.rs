use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bullet_exchange_interface::transaction::{RuntimeCall, TxDetails, UniquenessData, Version0};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::{SubmitTxRequest, SubmitTxResponse};
use crate::types::{CallMessage, SignedTransaction, UnsignedTransaction};
use crate::{Client, Keypair, SDKError, SDKResult};

impl Client {
    /// Build an unsigned transaction from a call message.
    ///
    /// Applies the client's default `max_fee`, `gas_limit`, and
    /// `max_priority_fee_bips` unless overridden per-transaction.
    ///
    /// # When to use this instead of `Transaction::builder()`
    ///
    /// Use this (together with `sign_transaction`) when you need to:
    /// - Build the transaction once, then sign it repeatedly in a loop
    ///   (e.g. a WebSocket order loop where the same unsigned tx is
    ///   re-signed on each keystroke)
    /// - Support hardware wallet / Ledger signing: build the unsigned tx,
    ///   send the raw bytes to the signer, receive the signature out-of-band,
    ///   and submit with `submit_transaction`
    ///
    /// For the common case of build + sign + submit in one step, prefer
    /// `Transaction::builder().call_message(...).send(&client).await`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Build once, sign repeatedly
    /// let unsigned = client.build_transaction(call_msg)?;
    /// loop {
    ///     let signed = client.sign_transaction(unsigned.clone(), &keypair)?;
    ///     client.submit_transaction(&signed).await?;
    /// }
    /// ```
    pub fn build_transaction(&self, call_msg: CallMessage) -> SDKResult<UnsignedTransaction> {
        let runtime_call = RuntimeCall::Exchange(call_msg);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SDKError::SystemTimeError)?
            .as_millis() as u64;
        let uniqueness = UniquenessData::Generation(timestamp);
        let details = TxDetails {
            chain_id: self.chain_id(),
            max_fee: self.max_fee(),
            gas_limit: self.gas_limit(),
            max_priority_fee_bips: self.max_priority_fee_bips(),
        };
        Ok(UnsignedTransaction {
            runtime_call,
            uniqueness,
            details,
        })
    }

    /// Sign an unsigned transaction with the given keypair.
    ///
    /// Returns a signed transaction ready for submission via `submit_transaction`.
    ///
    /// # When to use this
    ///
    /// Use when you need the signed transaction as a value — for example:
    /// - Signing the same unsigned tx repeatedly in a WebSocket order loop
    /// - Hardware wallet flows where `sign_transaction` is replaced by an
    ///   out-of-band signing call
    ///
    /// # Signing process
    ///
    /// 1. Borsh-serialize the unsigned transaction
    /// 2. Append the chain hash (32 bytes) as domain separator
    /// 3. Sign the concatenated bytes with Ed25519
    pub fn sign_transaction(
        &self,
        tx: UnsignedTransaction,
        keypair: &Keypair,
    ) -> SDKResult<SignedTransaction> {
        let mut data =
            borsh::to_vec(&tx).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        data.extend_from_slice(self.chain_hash());

        let sig_bytes = keypair.sign(&data);
        let signature: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|v: Vec<u8>| SDKError::InvalidSignatureLength(v.len()))?;

        let pk_bytes = keypair.public_key();
        let pub_key: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|v: Vec<u8>| SDKError::InvalidPublicKeyLength(v.len()))?;

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

    /// Encode a signed transaction as a base64 string for wire transmission.
    pub fn sign_to_base64(signed: &SignedTransaction) -> SDKResult<String> {
        let bytes =
            borsh::to_vec(&signed).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        Ok(BASE64.encode(&bytes))
    }

    /// Submit a signed transaction to the network.
    ///
    /// Returns the sequencer's response. For the common build + sign + submit
    /// flow, prefer `Transaction::builder().call_message(...).send(&client).await`.
    pub async fn submit_transaction(
        &self,
        signed: &SignedTransaction,
    ) -> SDKResult<SubmitTxResponse> {
        let body = Self::sign_to_base64(signed)?;
        let response = self.client().submit_tx(&SubmitTxRequest { body }).await?;
        Ok(response.into_inner())
    }

    /// Convenience method to sign and submit a transaction in one call.
    ///
    /// Equivalent to `build_transaction` + `sign_transaction` + `submit_transaction`.
    /// For most use cases, prefer `Transaction::builder()` which also provides a
    /// fluent interface for setting per-transaction fee overrides.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.sign_and_submit(call_msg, &keypair).await?;
    /// ```
    pub async fn sign_and_submit(
        &self,
        call_msg: CallMessage,
        keypair: &Keypair,
    ) -> SDKResult<SubmitTxResponse> {
        let unsigned = self.build_transaction(call_msg)?;
        let signed = self.sign_transaction(unsigned, keypair)?;
        self.submit_transaction(&signed).await
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "integration")]
    mod integration {
        use bullet_exchange_interface::message::PublicAction;

        use crate::types::CallMessage;
        use crate::{Client, Keypair, Network};

        #[tokio::test]
        async fn test_sign_apply_funding() {
            let network = std::env::var("BULLET_API_ENDPOINT")
                .map(|e| Network::from(e.as_str()))
                .unwrap_or(Network::Mainnet);

            let client = Client::builder()
                .network(network)
                .build()
                .await
                .expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg: CallMessage =
                CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            let unsigned = client
                .build_transaction(call_msg)
                .expect("Failed to build transaction");

            let signed = client
                .sign_transaction(unsigned, &keypair)
                .expect("Failed to sign transaction");

            assert!(!Client::sign_to_base64(&signed).unwrap().is_empty());
        }
    }
}
