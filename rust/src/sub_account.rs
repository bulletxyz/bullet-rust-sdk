//! Sub-account helpers.

use bullet_exchange_interface::address::Address;
use sha2::{Digest, Sha256};

/// Highest valid sub-account index. The runtime tracks sub-account existence in
/// a `u32` bitmask (`MasterV1 { sub_account_mask }`), so only `0..=31` slots
/// exist.
pub const MAX_SUB_ACCOUNT_INDEX: u8 = 31;

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
/// `Display`); `index` is the sub-account index. This mirrors the runtime's
/// `generate_sub_account_address`, which lives in the exchange crate (not
/// `bullet-exchange-interface`), so there is no shared source of truth to
/// import — it's duplicated here by necessity and locked by a golden-vector
/// test. If the runtime ever changes its seed scheme, this function and its
/// test must be updated to match.
///
/// Returns `Err` for an out-of-range `index` (`> `[`MAX_SUB_ACCOUNT_INDEX`]):
/// such an index can never correspond to a real sub-account, so deriving an
/// address for it would yield a valid-looking but meaningless address. Rejected
/// rather than silently returned.
pub fn derive_sub_account_address(master: &str, index: u8) -> Result<String, String> {
    if index > MAX_SUB_ACCOUNT_INDEX {
        return Err(format!(
            "sub-account index {index} out of range (0..={MAX_SUB_ACCOUNT_INDEX})"
        ));
    }
    let mut seed = master.as_bytes().to_vec();
    seed.push(index);
    let hash: [u8; 32] = Sha256::digest(&seed).into();
    Ok(Address(hash).to_string())
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
            derive_sub_account_address("default", 0).unwrap(),
            "DdKTBsowJD2b8UsevrwF73zNb3C2VjHuAG32VJzFcYc5"
        );
        assert_eq!(
            derive_sub_account_address("default", 1).unwrap(),
            "7Kfuk3KR19naCaswqS2w1Z7spwrzHDwyD5Ymirm6HMZj"
        );
    }

    #[test]
    fn rejects_out_of_range_index() {
        assert!(derive_sub_account_address("default", MAX_SUB_ACCOUNT_INDEX).is_ok());
        assert!(derive_sub_account_address("default", MAX_SUB_ACCOUNT_INDEX + 1).is_err());
        assert!(derive_sub_account_address("default", 255).is_err());
    }
}
