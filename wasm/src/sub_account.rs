use wasm_bindgen::prelude::*;

use crate::errors::WasmError;

/// Derive a sub-account's on-chain address from its master address and index.
/// @param {string} master - The master account's base58 address.
/// @param {number} index - The sub-account index (0-31).
/// @returns {string} The base58-encoded sub-account address.
/// @throws If `index` is out of range (> 31) — no such sub-account can exist.
/// @example
/// const sub = deriveSubAccountAddress(master, 1);
/// const account = await client.accountInfo(sub);
/// await client.sendCallMessage(User.placeOrders(marketId, orders, false, 1));
// `index` is `u32` (not `u8`) so a JS number outside `0..=255` is validated by
// the range check rather than silently truncated mod 256 at the JS→wasm boundary.
#[wasm_bindgen(js_name = deriveSubAccountAddress)]
pub fn derive_sub_account_address(master: &str, index: u32) -> Result<String, WasmError> {
    bullet_rust_sdk::derive_sub_account_address(master, index).map_err(WasmError::from)
}
