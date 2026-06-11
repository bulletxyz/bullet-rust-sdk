use wasm_bindgen::prelude::*;

/// Derive a sub-account's on-chain address from its master address and index.
///
/// Sub-account addresses are deterministic — the runtime seeds them with the
/// master address and index when handling `CreateSubAccount`. The master
/// account records only *which* indices exist (a bitmask on its account
/// variant), so compute the address client-side to read or switch to a
/// sub-account.
///
/// @param {string} master - The master account's base58 address.
/// @param {number} index - The sub-account index (0-31).
/// @returns {string} The base58-encoded sub-account address.
/// @throws If `index` is out of range (> 31) — no such sub-account can exist.
/// @example
/// const sub = deriveSubAccountAddress(master, 1);
/// const account = await client.accountInfo(sub);
/// await client.sendCallMessage(User.placeOrders(marketId, orders, false, 1));
#[wasm_bindgen(js_name = deriveSubAccountAddress)]
pub fn derive_sub_account_address(master: &str, index: u8) -> Result<String, JsError> {
    bullet_rust_sdk::derive_sub_account_address(master, index).map_err(|e| JsError::new(&e))
}
