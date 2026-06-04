pub mod decimal;

use crate::errors::WasmResult;

/// Convert a JS byte slice into a fixed-size `[u8; N]`, erroring with a
/// descriptive message (`name`) when the length doesn't match.
pub fn to_fixed_bytes<const N: usize>(bytes: &[u8], name: &str) -> WasmResult<[u8; N]> {
    Ok(bytes.try_into().map_err(|_| format!("expected {N}-byte {name}, got {}", bytes.len()))?)
}
