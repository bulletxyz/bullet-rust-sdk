//! Sub-account helpers.

use bullet_exchange_interface::address::Address;
use sha2::{Digest, Sha256};

/// Derive a sub-account's on-chain address from its master address and index.
///
/// Sub-account addresses are deterministic: the runtime seeds them with the
/// master address string followed by the index byte
/// (`sha256(master.to_string() ++ [index])`, taken as the 32-byte address) when
/// handling `CreateSubAccount`. The master account only records *which* indices
/// exist (a bitmask in its `MasterV1` variant), exposing no sub-account-address
/// lookup — so compute the address client-side to read or switch to a
/// sub-account.
///
/// `master` must be the master account's canonical base58 address string (its
/// `Display`); `index` is the sub-account index (0-31, bounded by the runtime's
/// `u32` sub-account mask). This mirrors the runtime's
/// `generate_sub_account_address`, which lives in the exchange crate (not
/// `bullet-exchange-interface`), so there is no shared source of truth to
/// import — it's duplicated here by necessity and locked by a golden-vector
/// test. If the runtime ever changes its seed scheme, this function and its
/// test must be updated to match.
pub fn derive_sub_account_address(master: &str, index: u8) -> String {
    let mut seed = master.as_bytes().to_vec();
    seed.push(index);
    let hash: [u8; 32] = Sha256::digest(&seed).into();
    Address(hash).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_sub_account_address_from_master_and_index() {
        // Golden vectors: base58(sha256("default" ++ [index])), computed
        // independently of this implementation (the same way the vault golden
        // test was). "default" is an opaque master stand-in — the function
        // treats `master` as bytes — and matches the runtime's
        // `generate_sub_account_address(master, index)`.
        assert_eq!(
            derive_sub_account_address("default", 0),
            "DdKTBsowJD2b8UsevrwF73zNb3C2VjHuAG32VJzFcYc5"
        );
        assert_eq!(
            derive_sub_account_address("default", 1),
            "7Kfuk3KR19naCaswqS2w1Z7spwrzHDwyD5Ymirm6HMZj"
        );
        // Distinct from the master and from each other.
        assert_ne!(
            derive_sub_account_address("default", 0),
            derive_sub_account_address("default", 1)
        );
    }
}
