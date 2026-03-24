use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use wasm_bindgen::prelude::*;

/// WASM wrapper for `rust_decimal::Decimal`.
///
/// Provides arbitrary-precision decimal arithmetic for JS. All arithmetic
/// methods return a new `WasmDecimal` so calls can be chained:
///
/// ```js
/// const total = price.mul(quantity).add(fee).round(2);
/// ```
#[wasm_bindgen(js_name = "Decimal")]
pub struct WasmDecimal(pub(crate) Decimal);

#[wasm_bindgen(js_class = "Decimal")]
impl WasmDecimal {
    // ── Constructors ─────────────────────────────────────────────────────

    /// Parse a decimal from a string (e.g. `"1.23456"`).
    #[wasm_bindgen(constructor)]
    pub fn new(value: &str) -> Result<WasmDecimal, String> {
        Decimal::from_str(value)
            .map(WasmDecimal)
            .map_err(|e| e.to_string())
    }

    /// Create a decimal from an integer.
    #[wasm_bindgen(js_name = fromI64)]
    pub fn from_i64(value: i64) -> WasmDecimal {
        WasmDecimal(Decimal::from(value))
    }

    /// Create a decimal from a JS number (f64). May lose precision.
    #[wasm_bindgen(js_name = fromF64)]
    pub fn from_f64(value: f64) -> Result<WasmDecimal, String> {
        Decimal::from_f64(value)
            .map(WasmDecimal)
            .ok_or_else(|| format!("cannot convert {value} to Decimal"))
    }

    /// The constant `0`.
    pub fn zero() -> WasmDecimal {
        WasmDecimal(Decimal::ZERO)
    }

    /// The constant `1`.
    pub fn one() -> WasmDecimal {
        WasmDecimal(Decimal::ONE)
    }

    // ── Arithmetic ───────────────────────────────────────────────────────

    pub fn add(&self, other: &WasmDecimal) -> WasmDecimal {
        WasmDecimal(self.0 + other.0)
    }

    pub fn sub(&self, other: &WasmDecimal) -> WasmDecimal {
        WasmDecimal(self.0 - other.0)
    }

    pub fn mul(&self, other: &WasmDecimal) -> WasmDecimal {
        WasmDecimal(self.0 * other.0)
    }

    pub fn div(&self, other: &WasmDecimal) -> Result<WasmDecimal, String> {
        if other.0.is_zero() {
            return Err("division by zero".to_string());
        }
        Ok(WasmDecimal(self.0 / other.0))
    }

    pub fn rem(&self, other: &WasmDecimal) -> Result<WasmDecimal, String> {
        if other.0.is_zero() {
            return Err("division by zero".to_string());
        }
        Ok(WasmDecimal(self.0 % other.0))
    }

    pub fn neg(&self) -> WasmDecimal {
        WasmDecimal(-self.0)
    }

    pub fn abs(&self) -> WasmDecimal {
        WasmDecimal(self.0.abs())
    }

    // ── Rounding ─────────────────────────────────────────────────────────

    /// Round to `dp` decimal places (half-up).
    pub fn round(&self, dp: u32) -> WasmDecimal {
        WasmDecimal(self.0.round_dp(dp))
    }

    /// Round down (toward zero) to `dp` decimal places.
    pub fn floor(&self, dp: u32) -> WasmDecimal {
        WasmDecimal(self.0.round_dp_with_strategy(dp, RoundingStrategy::ToZero))
    }

    /// Round up (away from zero) to `dp` decimal places.
    pub fn ceil(&self, dp: u32) -> WasmDecimal {
        WasmDecimal(self.0.round_dp_with_strategy(dp, RoundingStrategy::AwayFromZero))
    }

    // ── Comparison ───────────────────────────────────────────────────────

    pub fn eq(&self, other: &WasmDecimal) -> bool {
        self.0 == other.0
    }

    pub fn gt(&self, other: &WasmDecimal) -> bool {
        self.0 > other.0
    }

    pub fn gte(&self, other: &WasmDecimal) -> bool {
        self.0 >= other.0
    }

    pub fn lt(&self, other: &WasmDecimal) -> bool {
        self.0 < other.0
    }

    pub fn lte(&self, other: &WasmDecimal) -> bool {
        self.0 <= other.0
    }

    /// Returns -1, 0, or 1.
    pub fn cmp(&self, other: &WasmDecimal) -> i32 {
        match self.0.cmp(&other.0) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub fn min(&self, other: &WasmDecimal) -> WasmDecimal {
        WasmDecimal(self.0.min(other.0))
    }

    pub fn max(&self, other: &WasmDecimal) -> WasmDecimal {
        WasmDecimal(self.0.max(other.0))
    }

    // ── Predicates ───────────────────────────────────────────────────────

    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.0.is_sign_negative() && !self.0.is_zero()
    }

    // ── Conversion ───────────────────────────────────────────────────────

    /// String representation (e.g. `"1.23"`).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.0.to_string()
    }

    /// Convert to JS number (f64). May lose precision for large values.
    #[wasm_bindgen(js_name = toNumber)]
    pub fn to_number(&self) -> f64 {
        self.0.to_f64().unwrap_or(f64::NAN)
    }

    /// JSON serialization (as string to preserve precision).
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        self.0.to_string()
    }

    /// Number of decimal places.
    pub fn scale(&self) -> u32 {
        self.0.scale()
    }
}
