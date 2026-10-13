//! Vault helpers.

use bullet_exchange_interface::address::Address;
use sha2::{Digest, Sha256};

/// Derive a vault's on-chain address from its name.
///
/// `sha256(name)`, as the runtime seeds it on `CreateVault`. Neither the trading API nor
/// `CreateVault` events expose the address, so compute it client-side. `name` must match exactly
/// (no trimming or normalization).
///
/// Duplicates the exchange crate's `generate_address_with_seed` (not in
/// `bullet-exchange-interface`); a golden-vector test keeps them in sync.
pub fn derive_vault_address(name: &str) -> String {
    let hash: [u8; 32] = Sha256::digest(name.as_bytes()).into();
    Address(hash).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_vault_address_from_name() {
        // Golden vector computed independently; matches `generate_address_with_seed(b"default")`.
        assert_eq!(
            derive_vault_address("default"),
            "4kGq3HJ6gYLf5ekoFgZJ3hGAUuHP6sK1v2LPs5zKHaCn"
        );
    }
}
