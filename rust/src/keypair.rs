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

    /// The on-chain address (base58-encoded public key).
    ///
    /// This is the canonical address format used by the Bullet exchange.
    /// For the hex-encoded raw public key, see [`address_hex`](Self::address_hex).
    pub fn address(&self) -> String {
        let pk_bytes: [u8; 32] = self.signing_key.verifying_key().to_bytes();
        bullet_exchange_interface::address::Address(pk_bytes).to_string()
    }

    /// The public key as a hex string (32 bytes → 64 hex chars).
    pub fn address_hex(&self) -> String {
        hex::encode(self.public_key())
    }

    /// Write to a Solana-compatible JSON keystore file.
    ///
    /// Format: a JSON array of 64 integers — the 32-byte secret key followed
    /// by the 32-byte public key. Compatible with `solana-keygen` and Phantom.
    pub fn write_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let secret = self.signing_key.to_bytes();
        let public = self.signing_key.verifying_key().to_bytes();
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&secret);
        bytes[32..].copy_from_slice(&public);
        let json = serde_json::to_string(&bytes.as_slice())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Read a Solana-compatible JSON keystore file.
    ///
    /// Accepts either a 64-byte array (secret + public) or a 32-byte array
    /// (secret only). Returns an error if the file is missing or malformed.
    pub fn read_from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)?;
        let bytes: Vec<u8> = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if bytes.len() < 32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("keystore too short: {} bytes (need ≥32)", bytes.len()),
            ));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes[..32]);
        Ok(Self::from_bytes(secret))
    }
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair")
            .field("address", &self.address())
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
