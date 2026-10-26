//! Encoding the fast-bridge deposit memo.
//!
//! A Solana depositor bridging to Bullet may attach a memo naming where the funds
//! should land, instead of defaulting to their own wallet. The bytes produced here
//! go in the `memo` argument of Hyperlane's `TransferRemoteWithMemo`, and travel in
//! the warp `TokenMessage.metadata` tail.
//!
//! The codec itself is `bullet_exchange_interface::memo`, which is also what the
//! fast-bridge operator decodes with. Sharing it is the point: if the two could
//! drift, the operator would read these bytes as malformed and quietly pay the
//! depositor instead of the person they named.

use std::str::FromStr;

use bullet_exchange_interface::address::Address;
use bullet_exchange_interface::memo::{Memo, MemoV1, decode, encode};
use bullet_exchange_interface::message::TransferEndpoint;
use bullet_exchange_interface::types::BalanceBucket;

use crate::errors::{SDKError, SDKResult};
use crate::sub_account::MAX_SUB_ACCOUNT_INDEX;

/// Encode a memo directing a bridged deposit to `to_address`.
///
/// `sub_account_index` targets a sub-account rather than the master, and `balance`
/// picks the destination bucket — `Bank` for the wallet, or an exchange balance.
///
/// Returns bytes, not a `Result<_, TooLong>`: a v1 memo is at most 38 bytes against
/// a 256-byte limit, so there is no length to fail on. [`memo_fits_well_within_the_limit`]
/// pins that, and is the test to look at if a future version grows the format.
///
/// `sub_account_index` is a `u32` rather than a `u8` so that a JS caller naming
/// 32..=4294967295 is rejected here instead of wrapping into range. That is as
/// far as the type goes: a value JS can hold but a `u32` cannot — `NaN`, `3.9`,
/// `2**32 + 3` — is reshaped by wasm-bindgen's glue before Rust ever sees it, so
/// the wasm wrapper takes an `f64` and refuses those itself. See
/// `encode_fast_bridge_memo_wasm`.
///
/// Errors on a destination the shared decoder would refuse — the zero address, say —
/// rather than handing back a memo the operator will discard.
pub fn encode_fast_bridge_memo(
    to_address: &str,
    sub_account_index: Option<u32>,
    balance: BalanceBucket,
) -> SDKResult<Vec<u8>> {
    let to_address = Address::from_str(to_address)
        .map_err(|e| SDKError::InvalidAddress(format!("{to_address}: {e}")))?;
    let sub_account_index = match sub_account_index {
        Some(index) if index > MAX_SUB_ACCOUNT_INDEX => {
            return Err(SDKError::InvalidSubAccountIndex(index));
        }
        Some(index) => Some(index as u8), // checked above
        None => None,
    };
    let bytes = encode(&Memo::V1(MemoV1 {
        to_address,
        to: TransferEndpoint {
            sub_account_index,
            balance,
        },
    }));
    // Bytes the decoder refuses are worse than an error: the operator reads them as
    // "no memo" and pays the depositor, so the caller's chosen recipient is dropped
    // without anyone being told. Asking the real decoder — rather than restating its
    // rules, such as today's rejection of the zero address — is what keeps that
    // impossible when a future rule is added on the decoding side only.
    decode(&bytes)
        .map_err(|_| SDKError::InvalidMemo(format!("{to_address} is not a usable destination")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bullet_exchange_interface::memo::{MEMO_MAX_LEN, decode};
    use bullet_exchange_interface::types::MarketId;

    const ADDRESS: &str = "DdKTBsowJD2b8UsevrwF73zNb3C2VjHuAG32VJzFcYc5";

    /// Round-tripped through the operator's real decoder rather than a local
    /// replica, so this proves wire compatibility rather than self-consistency.
    #[test]
    fn round_trips_through_the_real_decoder() {
        let bytes = encode_fast_bridge_memo(ADDRESS, Some(3), BalanceBucket::Iso(MarketId(7)))
            .expect("encodes");
        let Memo::V1(decoded) = decode(&bytes).expect("decodes");
        assert_eq!(decoded.to_address, Address::from_str(ADDRESS).unwrap());
        assert_eq!(decoded.to.sub_account_index, Some(3));
        assert_eq!(decoded.to.balance, BalanceBucket::Iso(MarketId(7)));
    }

    /// The encoder returns bytes rather than a Result on length because this
    /// holds. If a future memo version breaks it, this test says so.
    #[test]
    fn memo_fits_well_within_the_limit() {
        for balance in [
            BalanceBucket::Bank,
            BalanceBucket::Cross,
            BalanceBucket::Spot,
            BalanceBucket::Iso(MarketId(u16::MAX)),
        ] {
            for index in [None, Some(MAX_SUB_ACCOUNT_INDEX)] {
                let bytes = encode_fast_bridge_memo(ADDRESS, index, balance).expect("encodes");
                assert!(
                    bytes.len() <= 38 && bytes.len() < MEMO_MAX_LEN,
                    "v1 memo grew to {} bytes",
                    bytes.len()
                );
            }
        }
    }

    #[test]
    fn rejects_an_unparseable_address() {
        assert!(matches!(
            encode_fast_bridge_memo("not-an-address", None, BalanceBucket::Bank),
            Err(SDKError::InvalidAddress(_))
        ));
    }

    /// Base58 for 32 zero bytes: parses as an address, but the shared decoder
    /// refuses it, so encoding it would hand back a memo the operator discards in
    /// favour of paying the depositor.
    #[test]
    fn rejects_a_destination_the_shared_decoder_would_refuse() {
        const ZERO: &str = "11111111111111111111111111111111";
        assert!(
            Address::from_str(ZERO).is_ok(),
            "the address parses; only the memo rule rejects it"
        );
        assert!(matches!(
            encode_fast_bridge_memo(ZERO, None, BalanceBucket::Bank),
            Err(SDKError::InvalidMemo(_))
        ));
    }

    #[test]
    fn rejects_a_sub_account_index_that_cannot_exist() {
        assert!(matches!(
            encode_fast_bridge_memo(ADDRESS, Some(32), BalanceBucket::Cross),
            Err(SDKError::InvalidSubAccountIndex(32))
        ));
    }
}
