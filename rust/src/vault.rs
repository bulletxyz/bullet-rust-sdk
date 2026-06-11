//! Vault helpers.

use bullet_exchange_interface::address::Address;
use sha2::{Digest, Sha256};

/// Derive a vault's on-chain address from its name.
///
/// Vault addresses are deterministic: the runtime seeds them with the vault
/// name (`sha256(name)`, taken as the 32-byte address) when handling
/// `CreateVault`. Computing it client-side lets you reference a vault right
/// after creating it — the trading API exposes no vault-address lookup, and
/// `CreateVault` emits no address-carrying event.
///
/// The name must match the one passed to `CreateVault` exactly (it is not
/// trimmed or normalized).
///
/// This mirrors the runtime's `generate_address_with_seed`, which lives in the
/// exchange crate (not `bullet-exchange-interface`), so there is no shared
/// source of truth to import — it's duplicated here by necessity. If the
/// runtime ever changes its seed scheme (a different hasher, a salt/prefix, or
/// name normalization), this function and its golden-vector test must be
/// updated to match.
pub fn derive_vault_address(name: &str) -> String {
    let hash: [u8; 32] = Sha256::digest(name.as_bytes()).into();
    Address(hash).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_vault_address_from_name() {
        // Golden vector: sha256("default") base58-encoded, computed
        // independently of this implementation. Matches the runtime's
        // `generate_address_with_seed(b"default")`.
        assert_eq!(derive_vault_address("default"), "4kGq3HJ6gYLf5ekoFgZJ3hGAUuHP6sK1v2LPs5zKHaCn");
    }
}
