use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bullet_exchange_interface::transaction::{
    Amount, PriorityFeeBips, RuntimeCall, TxDetails, UniquenessData, Version0,
};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::generated::types::{SubmitTxRequest, SubmitTxResponse};
use crate::types::{CallMessage, Transaction as SignedTransaction, UnsignedTransaction};
use crate::{Keypair, SDKError, SDKResult, TradingApi};

impl TradingApi {
    /// Build an unsigned transaction from a call message.
    ///
    /// This creates an unsigned transaction that can be signed with `sign_transaction`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let unsigned = client.build_transaction(call_msg, 10_000_000)?;
    /// let signed = client.sign_transaction(unsigned, &keypair)?;
    /// let response = client.submit_transaction(&signed).await?;
    /// ```
    pub fn build_transaction(
        &self,
        call_msg: CallMessage,
        max_fee: u128,
    ) -> SDKResult<UnsignedTransaction> {
        let runtime_call = RuntimeCall::Exchange(call_msg);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SDKError::SystemTimeError)?
            .as_millis() as u64;
        let uniqueness = UniquenessData::Generation(timestamp);
        let details = TxDetails {
            chain_id: self.chain_id(),
            max_fee: Amount(max_fee),
            gas_limit: None,
            max_priority_fee_bips: PriorityFeeBips(0),
        };
        Ok(UnsignedTransaction {
            runtime_call,
            uniqueness,
            details,
        })
    }

    /// Sign an unsigned transaction with the given keypair.
    ///
    /// Returns a signed transaction ready for submission.
    ///
    /// The signing process:
    /// 1. Borsh-serialize the unsigned transaction
    /// 2. Append the chain hash (32 bytes) as domain separator
    /// 3. Sign the combined bytes with ed25519
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

    pub fn sign_to_base64(signed: &SignedTransaction) -> SDKResult<String> {
        let bytes =
            borsh::to_vec(&signed).map_err(|e| SDKError::SerializationError(e.to_string()))?;
        Ok(BASE64.encode(&bytes))
    }

    /// Submit a signed transaction to the network.
    ///
    /// Returns the response from the sequencer.
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
    /// This is equivalent to calling `build_transaction`, `sign_transaction`,
    /// and `submit_transaction` in sequence.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.sign_and_submit(call_msg, 10_000_000, &keypair).await?;
    /// ```
    pub async fn sign_and_submit(
        &self,
        call_msg: CallMessage,
        max_fee: u128,
        keypair: &Keypair,
    ) -> SDKResult<SubmitTxResponse> {
        let unsigned = self.build_transaction(call_msg, max_fee)?;
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
        use crate::{Keypair, MAINNET_URL, TradingApi};

        #[tokio::test]
        async fn test_sign_apply_funding() {
            let endpoint = std::env::var("BULLET_API_ENDPOINT").unwrap_or(MAINNET_URL.to_string());

            let client = TradingApi::new(&endpoint, None)
                .await
                .expect("could not connect");
            let keypair = Keypair::generate();

            let call_msg: CallMessage =
                CallMessage::Public(PublicAction::ApplyFunding { addresses: vec![] });

            let unsigned = client
                .build_transaction(call_msg, 10_000_000)
                .expect("Failed to build transaction");

            let signed = client
                .sign_transaction(unsigned, &keypair)
                .expect("Failed to sign transaction");

            assert!(!TradingApi::sign_to_base64(&signed).unwrap().is_empty());
        }
    }
}
