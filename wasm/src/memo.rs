//! WASM wrapper for the fast-bridge deposit memo encoder.

use bullet_exchange_interface::types::{BalanceBucket as SdkBucket, MarketId};
use bullet_rust_sdk::{SDKError, encode_fast_bridge_memo};
use wasm_bindgen::prelude::*;

use crate::errors::WasmError;

/// Destination balance for a bridged deposit.
///
/// Fieldless so it crosses the JS boundary as a plain enum; `Iso` takes its market
/// through the separate `marketId` argument.
/// @enum {number}
#[wasm_bindgen(js_name = BalanceBucket)]
#[derive(Clone, Copy)]
pub enum WasmBalanceBucket {
    /// The recipient's on-chain wallet, outside the exchange.
    Bank,
    /// Cross-margin balance.
    Cross,
    /// Spot collateral balance.
    Spot,
    /// Isolated-margin balance for one market; requires `marketId`.
    Iso,
}

/// Encode a memo directing a bridged deposit to someone other than the depositor.
///
/// Pass the returned bytes as the `memo` argument of Hyperlane's
/// `TransferRemoteWithMemo` when bridging from Solana. Without a memo the deposit
/// is credited to the depositor's own wallet, which is the existing behaviour.
///
/// @param {string} toAddress - Recipient's base58 Bullet address.
/// @param {number | undefined} subAccountIndex - Sub-account to credit (0-31), or undefined for the master account.
/// @param {BalanceBucket} balance - Destination balance.
/// @param {number | undefined} marketId - Market to credit. Required for `Iso`, and rejected for every other balance.
/// @returns {Uint8Array} The encoded memo.
/// @throws If `toAddress` is not a valid address or not one a memo may name,
///   `subAccountIndex` or `marketId` is not a whole number in range (`NaN`,
///   fractions and values beyond the type are all refused, never rounded or
///   wrapped), `balance` is `Iso` without a `marketId`, or a `marketId` is given
///   with a balance other than `Iso`.
/// @example
/// const memo = encodeFastBridgeMemo(recipient, undefined, BalanceBucket.Cross);
#[wasm_bindgen(js_name = encodeFastBridgeMemo)]
pub fn encode_fast_bridge_memo_wasm(
    to_address: &str,
    sub_account_index: Option<f64>,
    balance: WasmBalanceBucket,
    market_id: Option<f64>,
) -> Result<Vec<u8>, WasmError> {
    // `f64`, not `u32`/`u16`: wasm-bindgen's glue for those emits `(x) >>> 0`, so
    // `NaN` (what `parseInt("")` yields) would arrive as sub-account 0 and `3.9`
    // as 3 — silently crediting a destination the caller never named.
    let sub_account_index = sub_account_index
        .map(|v| exact_index(v, "subAccountIndex", u32::MAX))
        .transpose()?;
    let market_id = market_id
        .map(|v| exact_index(v, "marketId", u16::MAX.into()).map(|v| v as u16))
        .transpose()?;
    // `SdkBucket::Iso` carries its market; splitting them for JS is what admits a
    // meaningless pair. Both halves are refused rather than resolved, because each
    // misdelivers money the caller cannot get back.
    let balance = match (balance, market_id) {
        (WasmBalanceBucket::Bank, None) => SdkBucket::Bank,
        (WasmBalanceBucket::Cross, None) => SdkBucket::Cross,
        (WasmBalanceBucket::Spot, None) => SdkBucket::Spot,
        (WasmBalanceBucket::Iso, Some(market_id)) => SdkBucket::Iso(MarketId(market_id)),
        // Defaulting the market would credit the wrong ledger.
        (WasmBalanceBucket::Iso, None) => {
            return Err(
                SDKError::InvalidMemo("balance Iso requires a marketId".to_string()).into(),
            );
        }
        // Dropping the marketId would encode a destination the caller did not ask
        // for: a caller naming a market meant an isolated balance, not this one.
        (WasmBalanceBucket::Bank | WasmBalanceBucket::Cross | WasmBalanceBucket::Spot, Some(_)) => {
            return Err(SDKError::InvalidMemo(
                "a marketId is only meaningful with balance Iso".to_string(),
            )
            .into());
        }
    };
    encode_fast_bridge_memo(to_address, sub_account_index, balance).map_err(WasmError::from)
}

/// Narrow a JS number to an exact integer in `0..=max`, refusing everything a
/// cast would have quietly accepted.
///
/// `max` is at most `u32::MAX`, far inside the range where an `f64` represents
/// every integer exactly, so the comparisons below are not approximate and the
/// cast afterwards cannot round.
fn exact_index(value: f64, field: &str, max: u32) -> Result<u32, WasmError> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(max) {
        return Err(SDKError::InvalidMemo(format!(
            "{field} must be a whole number in 0..={max}, got {value}"
        ))
        .into());
    }
    Ok(value as u32)
}
