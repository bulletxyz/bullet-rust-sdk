//! Keypair functionality for the Trading SDK.

use crate::errors::{SDKError, SDKResult};

/// An Ed25519 keypair for signing transactions.
///
/// This is a lightweight wrapper around ed25519_dalek::SigningKey that provides
/// convenient methods for creating keypairs and signing messages.
///
/// # Security Note
/// This stores the private key in memory. For production use with significant funds,
/// consider using a hardware wallet or external signing service.
#[derive(Clone)]
pub struct Keypair {
    signing_key: ed25519_dalek::SigningKey,
}

impl Keypair {
    /// Create a keypair from a 32-byte secret key.
    pub fn from_bytes(secret_key: [u8; 32]) -> Self {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key);
        Self { signing_key }
    }

    /// Create a keypair from a hex-encoded secret key.
    ///
    /// Accepts keys with or without "0x" prefix.
    pub fn from_hex(hex: &str) -> SDKResult<Self> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes: [u8; 32] = hex::decode(hex)
            .map_err(|e| SDKError::InvalidPrivateKey(e.to_string()))?
            .try_into()
            .map_err(|_| SDKError::InvalidPrivateKey("Expected 32 bytes".into()))?;
        Ok(Self::from_bytes(bytes))
    }

    /// Generate a new random keypair.
    ///
    /// Uses the OS random number generator.
    pub fn generate() -> Self {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Sign a message and return the 64-byte signature.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        let signature = self.signing_key.sign(message);
        signature.to_bytes().to_vec()
    }

    /// Get the 32-byte public key.
    pub fn public_key(&self) -> Vec<u8> {
        self.signing_key.verifying_key().as_bytes().to_vec()
    }

    /// Get the public key as hex string.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key())
    }
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair")
            .field("public_key", &self.public_key_hex())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_sign() {
        let keypair = Keypair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);
        assert_eq!(signature.len(), 64); // Ed25519 signatures are 64 bytes
    }

    #[test]
    fn test_from_hex() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let keypair = Keypair::from_hex(hex).unwrap();
        assert_eq!(keypair.public_key().len(), 32);
    }

    #[test]
    fn test_from_hex_with_prefix() {
        let hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let keypair = Keypair::from_hex(hex).unwrap();
        assert_eq!(keypair.public_key().len(), 32);
    }
}
