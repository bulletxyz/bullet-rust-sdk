use wasm_bindgen::prelude::*;

/// Derive a vault's on-chain address from its name.
///
/// Vault addresses are deterministic — the exchange seeds them with the vault
/// name when handling `CreateVault`. Compute it client-side to reference a
/// vault right after creating it (the trading API exposes no vault-address
/// lookup, and `CreateVault` emits no address-carrying event).
///
/// The name must match the one passed to `User.createVault` exactly.
///
/// @param {string} name - The vault name.
/// @returns {string} The base58-encoded vault address.
/// @example
/// const vault = deriveVaultAddress("My Vault");
/// await client.sendCallMessage(Vault.delegateVaultUserV1(vault, delegate, "bot", 0));
#[wasm_bindgen(js_name = deriveVaultAddress)]
pub fn derive_vault_address(name: &str) -> String {
    bullet_rust_sdk::derive_vault_address(name)
}
