use wasm_bindgen::prelude::*;

/// Wraps any SDK error and converts it into a JavaScript `Error` object.
///
/// Used as the error half of every `Result` returned through the wasm boundary.
/// Because it implements `Into<JsValue>`, `wasm-bindgen` accepts it directly in
/// `Result<T, WasmError>` return positions.
///
/// On the Rust side, any type that implements `std::fmt::Display` (which
/// includes all `thiserror`-derived errors) converts automatically via `?`:
///
/// ```rust,ignore
/// pub async fn foo(&self) -> Result<String, WasmError> {
///     let val = self.inner.some_sdk_call().await?;  // SDKError → WasmError
///     Ok(val.to_string())
/// }
/// ```
pub struct WasmError(String);

impl WasmError {
    pub fn new(msg: impl std::fmt::Display) -> Self {
        WasmError(msg.to_string())
    }
}

/// Convert to a JS `Error` object so the JS side gets `err.message` rather
/// than a plain string.
impl From<WasmError> for JsValue {
    fn from(e: WasmError) -> JsValue {
        js_sys::Error::new(&e.0).into()
    }
}

/// Blanket: any `Display` type (SDKError, WSErrors, serde_json::Error, …)
/// converts to `WasmError` automatically, so `?` just works.
impl<E: std::fmt::Display> From<E> for WasmError {
    fn from(e: E) -> Self {
        WasmError(e.to_string())
    }
}

/// Convenience alias — mirrors the `SDKResult<T>` pattern in the base crate.
pub type WasmResult<T> = Result<T, WasmError>;
