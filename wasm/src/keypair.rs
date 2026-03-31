use bullet_rust_sdk::Keypair;
use wasm_bindgen::prelude::*;

use crate::WasmResult;

/// Ed25519 keypair for signing transactions.
#[wasm_bindgen(js_name = Keypair)]
pub struct WasmKeypair {
    pub(crate) inner: Keypair,
}

#[wasm_bindgen(js_class = Keypair)]
impl WasmKeypair {
    /// Generate a new random keypair.
    pub fn generate() -> WasmKeypair {
        WasmKeypair {
            inner: Keypair::generate(),
        }
    }

    /// Create from a 32-byte hex private key (with or without `0x` prefix).
    #[wasm_bindgen(js_name = fromHex)]
    pub fn from_hex(hex: &str) -> WasmResult<WasmKeypair> {
        Ok(WasmKeypair {
            inner: Keypair::from_hex(hex)?,
        })
    }

    /// Create from a raw 32-byte `Uint8Array`.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> WasmResult<WasmKeypair> {
        let arr: [u8; 32] = bytes.try_into()?;
        Ok(WasmKeypair {
            inner: Keypair::from_bytes(arr),
        })
    }

    /// 32-byte secret key as `Uint8Array`.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Secret key as a lowercase hex string.
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.to_hex()
    }

    /// 32-byte public key as `Uint8Array`.
    #[wasm_bindgen(js_name = publicKey)]
    pub fn public_key(&self) -> Vec<u8> {
        self.inner.public_key()
    }

    /// Public key as a lowercase hex string.
    #[wasm_bindgen(js_name = publicKeyHex)]
    pub fn public_key_hex(&self) -> String {
        self.inner.public_key_hex()
    }

    /// Sign `message` and return the 64-byte Ed25519 signature as `Uint8Array`.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.inner.sign(message)
    }
}
