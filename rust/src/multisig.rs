//! Multisig (M-of-N) transaction support over the Solana offchain authenticator.
//!
//! Multisig transactions are submitted through the same spec-compliant Solana
//! offchain wire format used for Ledger signing, extended to N signers. The
//! formats mirror the Sovereign SDK's `sov-solana-offchain-auth` crate.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

use crate::{SDKError, SDKResult, UnsignedTransaction};

/// Maximum number of signers in a multisig, from the Sovereign SDK's
/// `MAX_SIGNERS` (the signer bitfield must fit a `u32`, and the Solana
/// offchain preamble caps total size).
pub const MAX_MULTISIG_SIGNERS: usize = 21;

// ── MultisigConfig ───────────────────────────────────────────────────────────

/// An M-of-N multisig signer set.
///
/// Public keys are canonicalized (sorted bytewise) on construction, matching
/// the Sovereign SDK's credential derivation. The same set of keys always
/// produces the same [`credential_id`](MultisigConfig::credential_id),
/// regardless of input order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultisigConfig {
    min_signers: u8,
    /// Sorted bytewise (canonical order).
    pubkeys: Vec<[u8; 32]>,
}

impl MultisigConfig {
    /// Create a multisig config from a threshold and a set of Ed25519 public
    /// keys.
    ///
    /// Requires `2..=21` distinct keys and `1 <= min_signers <= keys.len()`.
    pub fn new(min_signers: u8, mut pubkeys: Vec<[u8; 32]>) -> SDKResult<Self> {
        if pubkeys.len() < 2 || pubkeys.len() > MAX_MULTISIG_SIGNERS {
            return Err(SDKError::InvalidMultisig(format!(
                "expected 2-{MAX_MULTISIG_SIGNERS} signers, got {}",
                pubkeys.len()
            )));
        }
        pubkeys.sort_unstable();
        if pubkeys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SDKError::InvalidMultisig("duplicate public key".to_string()));
        }
        for pubkey in &pubkeys {
            // Reject keys that aren't valid Ed25519 points — they could never
            // produce a verifying signature, making the config uncompletable.
            ed25519_dalek::VerifyingKey::from_bytes(pubkey).map_err(|_| {
                SDKError::InvalidMultisig(format!(
                    "invalid Ed25519 public key: {}",
                    bs58::encode(pubkey).into_string()
                ))
            })?;
        }
        if min_signers == 0 || min_signers as usize > pubkeys.len() {
            return Err(SDKError::InvalidMultisig(format!(
                "min_signers must be 1-{}, got {min_signers}",
                pubkeys.len()
            )));
        }
        Ok(Self { min_signers, pubkeys })
    }

    /// The signature threshold (the M in M-of-N).
    pub fn min_signers(&self) -> u8 {
        self.min_signers
    }

    /// The signer public keys in canonical (bytewise sorted) order.
    pub fn pubkeys(&self) -> &[[u8; 32]] {
        &self.pubkeys
    }

    /// The 32-byte credential id of this multisig:
    /// `sha256(min_signers_u8 || borsh(sorted pubkeys))`.
    ///
    /// This is the account identity the rollup derives for the multisig.
    pub fn credential_id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.min_signers.to_le_bytes());
        hasher.update(
            borsh::to_vec(&self.pubkeys).expect("serializing Vec<[u8; 32]> to vec is infallible"),
        );
        hasher.finalize().into()
    }

    /// The base58-encoded credential id, as committed to in the signed
    /// multisig message (`multisig_id` field).
    pub fn multisig_id(&self) -> String {
        bs58::encode(self.credential_id()).into_string()
    }

    /// Index of `pubkey` in canonical order, if it is part of the set.
    pub(crate) fn signer_index(&self, pubkey: &[u8; 32]) -> Option<usize> {
        self.pubkeys.iter().position(|key| key == pubkey)
    }
}

// ── Multisig signable bytes ──────────────────────────────────────────────────

impl UnsignedTransaction {
    /// Serialize into the JSON message every multisig signer must commit to.
    ///
    /// Extends [`to_message_bytes`](UnsignedTransaction::to_message_bytes)
    /// with the `multisig_id` (base58 credential id) and `version: 1` fields
    /// required by the V1 Solana offchain payload. All signers of the same
    /// transaction and config produce identical bytes.
    pub fn to_multisig_message_bytes(&self, config: &MultisigConfig) -> SDKResult<Vec<u8>> {
        let bytes = self.to_message_bytes()?;
        let mut message: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&bytes)?;
        message.insert("multisig_id".to_string(), serde_json::Value::String(config.multisig_id()));
        message.insert("version".to_string(), serde_json::Value::from(1u8));
        serde_json::to_vec(&message).map_err(Into::into)
    }

    /// Build the bytes each multisig signer (e.g. a Ledger hardware wallet)
    /// must sign.
    ///
    /// Prepends the spec-compliant Solana off-chain preamble — carrying all
    /// N signer pubkeys — to the multisig JSON message. Collect the resulting
    /// signatures with [`SolanaLedgerMultisigTransaction::add_signature`] and
    /// submit via `Client::send_ledger_multisig_transaction`.
    pub fn to_ledger_multisig_signable_bytes(&self, config: &MultisigConfig) -> SDKResult<Vec<u8>> {
        let json_bytes = self.to_multisig_message_bytes(config)?;
        let message_len = u16::try_from(json_bytes.len()).map_err(|_| {
            SDKError::SerializationError(format!(
                "JSON message too large for Solana preamble: {} bytes (max {})",
                json_bytes.len(),
                u16::MAX
            ))
        })?;
        let mut result = crate::transaction_builder::make_solana_preamble(
            config.pubkeys(),
            &self.chain_hash,
            message_len,
        );
        result.extend_from_slice(&json_bytes);
        Ok(result)
    }
}

// ── Multisig transaction assembly ────────────────────────────────────────────

/// A multisig transaction collecting signatures over the spec-compliant Solana
/// offchain wire format (the format Ledger hardware wallets sign).
///
/// Every signer must sign the exact same bytes. Distribute the bytes from
/// [`signable_bytes`](Self::signable_bytes) to each signer rather than having
/// each rebuild the transaction independently — the signed message embeds the
/// chain hash and a JSON payload whose byte representation must match exactly
/// for the signatures to verify against one credential. Once
/// [`is_complete`](Self::is_complete), submit via
/// [`Client::send_ledger_multisig_transaction`].
///
/// The signable bytes bake in the chain hash at construction time, so if the
/// chain hash rotates (a schema update) before enough signatures are collected,
/// submission fails with [`SDKError::TransactionOutdated`] and the transaction
/// must be rebuilt and re-signed by every signer. The default `Window`
/// uniqueness needs no chain round-trip; override via `.uniqueness(...)` only
/// if you need a specific value.
///
/// ```ignore
/// let unsigned = UnsignedTransaction::builder()
///     .call_message(call_msg)
///     .max_fee(10_000_000)
///     .client(&client)
///     .build()?;
///
/// let mut tx = SolanaLedgerMultisigTransaction::new(unsigned, config)?;
/// tx.add_signature(pubkey_a, signature_a)?;
/// tx.add_signature(pubkey_b, signature_b)?;
/// client.send_ledger_multisig_transaction(&tx).await?;
/// ```
#[derive(Clone, Debug)]
pub struct SolanaLedgerMultisigTransaction {
    /// Preamble (with all N pubkeys) + multisig JSON message. Signed by every
    /// signer.
    signed_message: Vec<u8>,
    config: MultisigConfig,
    /// Collected signatures, keyed by the signer's canonical index.
    signatures: std::collections::BTreeMap<usize, [u8; 64]>,
}

impl SolanaLedgerMultisigTransaction {
    /// Create a multisig transaction from an unsigned transaction and the
    /// signer set.
    pub fn new(tx: UnsignedTransaction, config: MultisigConfig) -> SDKResult<Self> {
        Ok(Self {
            signed_message: tx.to_ledger_multisig_signable_bytes(&config)?,
            config,
            signatures: std::collections::BTreeMap::new(),
        })
    }

    /// The bytes every signer must sign (preamble + JSON message).
    pub fn signable_bytes(&self) -> &[u8] {
        &self.signed_message
    }

    /// The multisig signer set.
    pub fn config(&self) -> &MultisigConfig {
        &self.config
    }

    /// Add one signer's Ed25519 signature over
    /// [`signable_bytes`](Self::signable_bytes).
    ///
    /// Rejects signers outside the configured set, duplicate signatures, and
    /// signatures that do not verify against the signable bytes.
    pub fn add_signature(&mut self, pubkey: [u8; 32], signature: [u8; 64]) -> SDKResult<()> {
        let index = self.config.signer_index(&pubkey).ok_or_else(|| {
            SDKError::InvalidMultisig(format!(
                "signer {} is not part of the multisig",
                bs58::encode(pubkey).into_string()
            ))
        })?;
        if self.signatures.contains_key(&index) {
            return Err(SDKError::InvalidMultisig(format!(
                "signer {} has already signed",
                bs58::encode(pubkey).into_string()
            )));
        }

        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pubkey)
            .map_err(|e| SDKError::InvalidMultisig(format!("invalid public key: {e}")))?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature);
        ed25519_dalek::Verifier::verify(&verifying_key, &self.signed_message, &signature).map_err(
            |_| {
                SDKError::InvalidMultisig(format!(
                    "signature from {} does not verify against the signable bytes",
                    bs58::encode(pubkey).into_string()
                ))
            },
        )?;

        self.signatures.insert(index, signature.to_bytes());
        Ok(())
    }

    /// Number of signatures collected so far.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Whether enough signatures have been collected to meet the threshold.
    pub fn is_complete(&self) -> bool {
        self.signatures.len() >= self.config.min_signers() as usize
    }

    /// Serialize to the raw binary wire format.
    ///
    /// Borsh layout, matching the Sovereign SDK's
    /// `SolanaOffchainSpecCompliantMultisigMessage`:
    /// `[u32 LE: message len][signed_message][u32 LE: signature count]
    ///  [64-byte signatures in canonical index order][u32 LE: signer bitfield]
    ///  [u8: min_signers]`
    ///
    /// Errors if the threshold has not been met.
    pub fn to_bytes(&self) -> SDKResult<Vec<u8>> {
        if !self.is_complete() {
            return Err(SDKError::InvalidMultisig(format!(
                "not enough signatures: need {}, got {}",
                self.config.min_signers(),
                self.signatures.len()
            )));
        }

        let mut bitfield: u32 = 0;
        for index in self.signatures.keys() {
            bitfield |= 1 << index;
        }

        let mut buf =
            Vec::with_capacity(4 + self.signed_message.len() + 4 + self.signatures.len() * 64 + 5);
        buf.extend_from_slice(&(self.signed_message.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.signed_message);
        buf.extend_from_slice(&(self.signatures.len() as u32).to_le_bytes());
        // BTreeMap iterates in ascending index order, matching the bitfield
        // bits from LSB to MSB.
        for signature in self.signatures.values() {
            buf.extend_from_slice(signature);
        }
        buf.extend_from_slice(&bitfield.to_le_bytes());
        buf.push(self.config.min_signers());
        Ok(buf)
    }

    /// Serialize to wire format and base64-encode for submission.
    pub fn to_base64(&self) -> SDKResult<String> {
        Ok(BASE64.encode(self.to_bytes()?))
    }
}

// ── Client methods ───────────────────────────────────────────────────────────

impl crate::Client {
    /// Send a completed multisig transaction to the network.
    ///
    /// Submits the spec-compliant multisig wire format to the trading API's
    /// `/api/v1/solanaOffchainTx`. The transaction must have collected at least
    /// [`MultisigConfig::min_signers`] signatures.
    pub async fn send_ledger_multisig_transaction(
        &self,
        signed: &SolanaLedgerMultisigTransaction,
    ) -> SDKResult<crate::SubmitTxResponse> {
        self.submit_offchain(signed.to_base64()?).await
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use bullet_exchange_interface::message::PublicAction;
    use bullet_exchange_interface::transaction::{
        Amount, PriorityFeeBips, RuntimeCall, TxDetails, UniquenessData,
        UnsignedTransaction as RawUnsignedTransaction,
    };

    use super::*;
    use crate::types::CallMessage;
    use crate::{Keypair, SDKError, UnsignedTransaction};

    /// Three real Ed25519 pubkeys derived from fixed seeds, returned out of
    /// canonical (bytewise) order. Derived from seeds so the golden vectors
    /// below can be reproduced with the JS reference (`@noble/ed25519`).
    fn test_keys() -> Vec<[u8; 32]> {
        let mut seed3 = [0u8; 32];
        seed3[0] = 3;
        [[2u8; 32], [1u8; 32], seed3]
            .into_iter()
            .map(|seed| Keypair::from_bytes(seed).public_key().try_into().unwrap())
            .collect()
    }

    fn test_config() -> MultisigConfig {
        MultisigConfig::new(2, test_keys()).unwrap()
    }

    fn test_unsigned_tx() -> UnsignedTransaction {
        let inner = RawUnsignedTransaction {
            runtime_call: RuntimeCall::Exchange(CallMessage::Public(PublicAction::ApplyFunding {
                addresses: vec![],
            })),
            uniqueness: UniquenessData::Nonce(5),
            details: TxDetails {
                chain_id: 1,
                max_fee: Amount(10_000_000),
                gas_limit: None,
                max_priority_fee_bips: PriorityFeeBips(0),
            },
        };
        UnsignedTransaction { inner, chain_hash: [42u8; 32], chain_name: "TestChain".to_string() }
    }

    #[test]
    fn multisig_message_bytes_commit_to_multisig_id_and_version() {
        let unsigned = test_unsigned_tx();
        let config = test_config();

        let bytes = unsigned.to_multisig_message_bytes(&config).unwrap();
        let message: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(message["multisig_id"], config.multisig_id());
        assert_eq!(message["version"], 1);
        assert_eq!(message["chain_name"], "TestChain");
        assert_eq!(message["uniqueness"]["nonce"], 5);
        assert_eq!(message["details"]["max_fee"], "10000000");
        assert!(message.get("runtime_call").is_some());
    }

    #[test]
    fn multisig_message_bytes_field_order_matches_sovereign_js() {
        // Signers must all produce byte-identical messages; the JS SDK emits
        // keys in this insertion order.
        let bytes = test_unsigned_tx().to_multisig_message_bytes(&test_config()).unwrap();
        let message: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&bytes).unwrap();

        let keys: Vec<&str> = message.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            ["runtime_call", "uniqueness", "details", "chain_name", "multisig_id", "version"]
        );
    }

    #[test]
    fn multisig_preamble_layout_matches_sovereign_reference() {
        let config = test_config();
        let chain_hash = [7u8; 32];
        let message_length: u16 = 0x1234;

        let preamble = crate::transaction_builder::make_solana_preamble(
            config.pubkeys(),
            &chain_hash,
            message_length,
        );

        assert_eq!(preamble.len(), 53 + 3 * 32);
        // signing domain
        assert_eq!(preamble[0], 0xff);
        assert_eq!(&preamble[1..16], b"solana offchain");
        // header_version = 0
        assert_eq!(preamble[16], 0);
        // application_domain = chain_hash
        assert_eq!(&preamble[17..49], &chain_hash);
        // message_format = 0
        assert_eq!(preamble[49], 0);
        // signer_count = 3
        assert_eq!(preamble[50], 3);
        // pubkeys in canonical order
        assert_eq!(&preamble[51..83], &config.pubkeys()[0]);
        assert_eq!(&preamble[83..115], &config.pubkeys()[1]);
        assert_eq!(&preamble[115..147], &config.pubkeys()[2]);
        // message_length LE
        assert_eq!(&preamble[147..149], &message_length.to_le_bytes());
    }

    #[test]
    fn ledger_multisig_signable_bytes_are_preamble_plus_json() {
        let unsigned = test_unsigned_tx();
        let config = test_config();

        let json_bytes = unsigned.to_multisig_message_bytes(&config).unwrap();
        let signable = unsigned.to_ledger_multisig_signable_bytes(&config).unwrap();

        let preamble_len = 53 + 3 * 32;
        assert_eq!(signable.len(), preamble_len + json_bytes.len());
        assert_eq!(&signable[preamble_len..], json_bytes.as_slice());
        assert_eq!(signable[0], 0xff);
        assert_eq!(signable[50], 3); // signer_count
    }

    /// Three generated keypairs plus a 2-of-3 config over their pubkeys.
    /// Returns the keypairs sorted in canonical (bytewise pubkey) order.
    fn test_signers() -> (Vec<Keypair>, MultisigConfig) {
        let mut keypairs: Vec<Keypair> = (0..3).map(|_| Keypair::generate()).collect();
        keypairs.sort_by_key(|kp| kp.public_key());
        let pubkeys: Vec<[u8; 32]> =
            keypairs.iter().map(|kp| kp.public_key().try_into().unwrap()).collect();
        (keypairs, MultisigConfig::new(2, pubkeys).unwrap())
    }

    fn sign_with(keypair: &Keypair, message: &[u8]) -> ([u8; 32], [u8; 64]) {
        let pubkey: [u8; 32] = keypair.public_key().try_into().unwrap();
        let signature: [u8; 64] = keypair.sign(message).try_into().unwrap();
        (pubkey, signature)
    }

    #[test]
    fn multisig_transaction_signable_bytes_match_unsigned_tx() {
        let (_, config) = test_signers();
        let unsigned = test_unsigned_tx();

        let expected = unsigned.to_ledger_multisig_signable_bytes(&config).unwrap();
        let tx = SolanaLedgerMultisigTransaction::new(unsigned, config).unwrap();

        assert_eq!(tx.signable_bytes(), expected.as_slice());
    }

    #[test]
    fn multisig_transaction_tracks_threshold_completion() {
        let (keypairs, config) = test_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        assert_eq!(tx.signature_count(), 0);
        assert!(!tx.is_complete());

        let (pk, sig) = sign_with(&keypairs[0], tx.signable_bytes());
        tx.add_signature(pk, sig).unwrap();
        assert_eq!(tx.signature_count(), 1);
        assert!(!tx.is_complete());

        let (pk, sig) = sign_with(&keypairs[2], tx.signable_bytes());
        tx.add_signature(pk, sig).unwrap();
        assert_eq!(tx.signature_count(), 2);
        assert!(tx.is_complete());
    }

    #[test]
    fn multisig_wire_format_orders_signatures_by_canonical_index() {
        let (keypairs, config) = test_signers();
        let min_signers = config.min_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        // Sign with canonical indices 2 and 0, added out of order.
        let (pk2, sig2) = sign_with(&keypairs[2], tx.signable_bytes());
        let (pk0, sig0) = sign_with(&keypairs[0], tx.signable_bytes());
        tx.add_signature(pk2, sig2).unwrap();
        tx.add_signature(pk0, sig0).unwrap();

        let wire = tx.to_bytes().unwrap();

        // [u32 LE: signed_message len][signed_message]
        let msg_len = u32::from_le_bytes(wire[0..4].try_into().unwrap()) as usize;
        assert_eq!(&wire[4..4 + msg_len], tx.signable_bytes());
        let mut offset = 4 + msg_len;

        // [u32 LE: signature count][64-byte signatures in index order]
        let sig_count = u32::from_le_bytes(wire[offset..offset + 4].try_into().unwrap());
        assert_eq!(sig_count, 2);
        offset += 4;
        assert_eq!(&wire[offset..offset + 64], &sig0);
        offset += 64;
        assert_eq!(&wire[offset..offset + 64], &sig2);
        offset += 64;

        // [u32 LE: signer bitfield] — signers 0 and 2 → 0b101
        let bitfield = u32::from_le_bytes(wire[offset..offset + 4].try_into().unwrap());
        assert_eq!(bitfield, 0b101);
        offset += 4;

        // [u8: min_signers]
        assert_eq!(wire[offset], min_signers);
        assert_eq!(wire.len(), offset + 1);
    }

    #[test]
    fn multisig_add_signature_rejects_unknown_signer() {
        let (_, config) = test_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        let outsider = Keypair::generate();
        let (pk, sig) = sign_with(&outsider, tx.signable_bytes());
        let err = tx.add_signature(pk, sig).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn multisig_add_signature_rejects_duplicate_signer() {
        let (keypairs, config) = test_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        let (pk, sig) = sign_with(&keypairs[0], tx.signable_bytes());
        tx.add_signature(pk, sig).unwrap();
        let err = tx.add_signature(pk, sig).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn multisig_add_signature_rejects_invalid_signature() {
        let (keypairs, config) = test_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        let pubkey: [u8; 32] = keypairs[0].public_key().try_into().unwrap();
        let err = tx.add_signature(pubkey, [9u8; 64]).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn multisig_to_bytes_errors_below_threshold() {
        let (keypairs, config) = test_signers();
        let mut tx = SolanaLedgerMultisigTransaction::new(test_unsigned_tx(), config).unwrap();

        let err = tx.to_bytes().unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");

        let (pk, sig) = sign_with(&keypairs[1], tx.signable_bytes());
        tx.add_signature(pk, sig).unwrap();
        let err = tx.to_bytes().unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn ledger_multisig_signable_bytes_errors_when_json_exceeds_u16() {
        let mut unsigned = test_unsigned_tx();
        unsigned.inner.runtime_call =
            RuntimeCall::Exchange(CallMessage::Public(PublicAction::ApplyFunding {
                addresses: vec![bullet_exchange_interface::address::Address([0u8; 32]); 3000],
            }));

        let err = unsigned.to_ledger_multisig_signable_bytes(&test_config()).unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn credential_id_matches_sovereign_js_reference() {
        // Golden vector computed with the admin panel / @sovereign-sdk JS
        // implementation: sha256(threshold_u8 || borsh(sorted pubkeys)).
        let config = MultisigConfig::new(2, test_keys()).unwrap();

        assert_eq!(
            hex::encode(config.credential_id()),
            "f316a57cd06be916c2c51677163de282c53b80d85b3208d680f6e9448b25c56b"
        );
    }

    #[test]
    fn multisig_id_is_base58_of_credential_id() {
        let config = MultisigConfig::new(2, test_keys()).unwrap();

        assert_eq!(config.multisig_id(), "HMv6kdvx7sVBr59PJXfpeHYYoMktAhYP5iX41V4KakFC");
    }

    #[test]
    fn pubkeys_are_canonicalized_bytewise() {
        let config = MultisigConfig::new(2, test_keys()).unwrap();

        let mut expected = test_keys();
        expected.sort_unstable();
        assert_eq!(config.pubkeys(), expected.as_slice());

        // Input order must not affect the credential id.
        let mut reversed = test_keys();
        reversed.reverse();
        let config2 = MultisigConfig::new(2, reversed).unwrap();
        assert_eq!(config.credential_id(), config2.credential_id());
    }

    #[test]
    fn rejects_too_few_or_too_many_signers() {
        let err = MultisigConfig::new(1, vec![[1u8; 32]]).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");

        let many: Vec<[u8; 32]> = (0..22u8)
            .map(|i| {
                let mut k = [0u8; 32];
                k[0] = i;
                k
            })
            .collect();
        let err = MultisigConfig::new(2, many).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn rejects_duplicate_pubkeys() {
        let err = MultisigConfig::new(2, vec![[1u8; 32], [1u8; 32], [2u8; 32]]).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }

    #[test]
    fn rejects_invalid_min_signers() {
        let err = MultisigConfig::new(0, test_keys()).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");

        let err = MultisigConfig::new(4, test_keys()).unwrap_err();
        assert!(matches!(err, SDKError::InvalidMultisig(_)), "{err:?}");
    }
}
