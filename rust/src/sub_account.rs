//! Sub-account helpers.

use bullet_exchange_interface::address::Address;
use sha2::{Digest, Sha256};

use crate::errors::{SDKError, SDKResult};

/// Highest valid sub-account index; the runtime's `sub_account_mask` is a `u32`.
pub const MAX_SUB_ACCOUNT_INDEX: u32 = 31;

/// Derive a sub-account's on-chain address from its master address and index.
///
/// Uses `sha256(master.to_string() ++ [index])`, matching the runtime's address derivation.
/// Returns [`SDKError::InvalidSubAccountIndex`] for indices above 31.
pub fn derive_sub_account_address(master: &str, index: u32) -> SDKResult<String> {
    if index > MAX_SUB_ACCOUNT_INDEX {
        return Err(SDKError::InvalidSubAccountIndex(index));
    }
    let mut seed = master.as_bytes().to_vec();
    seed.push(index as u8); // safe: validated <= 31 above
    let hash: [u8; 32] = Sha256::digest(&seed).into();
    Ok(Address(hash).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_sub_account_address_from_master_and_index() {
        // Golden vectors: base58(sha256("default" ++ [index])), computed independently and matching
        // the runtime's `generate_sub_account_address`.
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
        // Values that would wrap if narrowed to u8 (257 % 256 == 1) must still
        // be rejected — the u32 param prevents the silent truncation.
        assert!(derive_sub_account_address("default", 257).is_err());
        assert!(derive_sub_account_address("default", 288).is_err()); // 288 % 256 == 32
    }
}
