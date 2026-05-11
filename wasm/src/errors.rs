use bullet_rust_sdk::{ApiErrorResponse, ManagedWsError, SDKError, WSErrors};
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(raw_module = "./bullet-sdk-error.js")]
extern "C" {
    #[wasm_bindgen(js_name = BulletSdkError)]
    type JsBulletSdkError;

    #[wasm_bindgen(constructor, js_class = BulletSdkError)]
    fn new(message: &str, options: &JsValue) -> JsBulletSdkError;
}

/// Frontend-facing category attached to `BulletSdkError.kind`.
#[derive(Clone, Copy)]
enum WasmErrorKind {
    Api,
    Http,
    Websocket,
    Validation,
    Serialization,
    Network,
    Unknown,
}

impl WasmErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            WasmErrorKind::Api => "api",
            WasmErrorKind::Http => "http",
            WasmErrorKind::Websocket => "websocket",
            WasmErrorKind::Validation => "validation",
            WasmErrorKind::Serialization => "serialization",
            WasmErrorKind::Network => "network",
            WasmErrorKind::Unknown => "unknown",
        }
    }
}

/// Wraps SDK errors and converts them into JavaScript `BulletSdkError` objects.
///
/// Used as the error half of every `Result` returned through the wasm boundary.
/// Because it implements `Into<JsValue>`, `wasm-bindgen` accepts it directly in
/// `Result<T, WasmError>` return positions.
pub struct WasmError {
    message: String,
    kind: WasmErrorKind,
    status: Option<u16>,
    details: Option<JsValue>,
    retryable: bool,
}

impl WasmError {
    pub fn new(msg: impl std::fmt::Display) -> Self {
        WasmError {
            message: msg.to_string(),
            kind: WasmErrorKind::Unknown,
            status: None,
            details: None,
            retryable: false,
        }
    }

    fn with_kind(mut self, kind: WasmErrorKind) -> Self {
        self.kind = kind;
        self
    }
}

fn set_property(target: &JsValue, key: &str, value: &JsValue) {
    let _ = Reflect::set(target, &JsValue::from_str(key), value);
}

/// Convert to a JS `BulletSdkError` object so the JS side gets `err.message`
/// plus parseable metadata such as `kind`, `status`, `details`, and `retryable`.
impl From<WasmError> for JsValue {
    fn from(e: WasmError) -> JsValue {
        let options: JsValue = Object::new().into();

        set_property(&options, "kind", &JsValue::from_str(e.kind.as_str()));
        set_property(&options, "retryable", &JsValue::from_bool(e.retryable));

        if let Some(status) = e.status {
            set_property(&options, "status", &JsValue::from_f64(status as f64));
        }

        if let Some(details) = e.details {
            set_property(&options, "details", &details);
        }

        JsBulletSdkError::new(&e.message, &options).into()
    }
}

impl From<SDKError> for WasmError {
    fn from(e: SDKError) -> Self {
        let retryable = e.is_retryable();
        let kind = match &e {
            SDKError::InvalidNetwork | SDKError::InvalidNetworkUrl => WasmErrorKind::Network,
            SDKError::InvalidPrivateKey(_)
            | SDKError::MissingKeypair
            | SDKError::InvalidSchemaResponse(_)
            | SDKError::InvalidChainHash(_)
            | SDKError::InvalidSignatureLength(_)
            | SDKError::InvalidPublicKeyLength(_)
            | SDKError::UnsupportedCallMessage(_)
            | SDKError::TransactionOutdated
            | SDKError::RequestError(_) => WasmErrorKind::Validation,
            SDKError::JsonSerializeError(_)
            | SDKError::StringParseError(_)
            | SDKError::SerializationError(_) => WasmErrorKind::Serialization,
            SDKError::HttpError(_) => WasmErrorKind::Http,
            SDKError::ApiError(_) => WasmErrorKind::Api,
            SDKError::WebsocketError(_) => WasmErrorKind::Websocket,
            SDKError::ChainIdCastError(_)
            | SDKError::SystemTimeError
            | SDKError::SchemaOutdated => WasmErrorKind::Unknown,
            _ => WasmErrorKind::Unknown,
        };

        let mut err = WasmError::new(e.to_string()).with_kind(kind);
        err.retryable = retryable;

        if let Some(api_error) = e.api_error() {
            err.status = Some(api_error.status);
            err.details = api_error
                .details
                .as_ref()
                .map(|details| serde_wasm_bindgen::to_value(details).unwrap_or(JsValue::NULL));
        }

        err
    }
}

impl From<bullet_rust_sdk::codegen::Error<ApiErrorResponse>> for WasmError {
    fn from(e: bullet_rust_sdk::codegen::Error<ApiErrorResponse>) -> Self {
        WasmError::from(SDKError::from(e))
    }
}

impl From<reqwest::Error> for WasmError {
    fn from(e: reqwest::Error) -> Self {
        let retryable = e.is_timeout() || e.is_request();
        let mut err = WasmError::new(e).with_kind(WasmErrorKind::Http);
        err.retryable = retryable;
        err
    }
}

impl From<WSErrors> for WasmError {
    fn from(e: WSErrors) -> Self {
        WasmError::from(SDKError::from(e))
    }
}

impl From<ManagedWsError> for WasmError {
    fn from(e: ManagedWsError) -> Self {
        WasmError::new(e).with_kind(WasmErrorKind::Websocket)
    }
}

impl From<serde_json::Error> for WasmError {
    fn from(e: serde_json::Error) -> Self {
        WasmError::new(e).with_kind(WasmErrorKind::Serialization)
    }
}

impl From<rust_decimal::Error> for WasmError {
    fn from(e: rust_decimal::Error) -> Self {
        WasmError::new(e).with_kind(WasmErrorKind::Validation)
    }
}

impl From<std::array::TryFromSliceError> for WasmError {
    fn from(e: std::array::TryFromSliceError) -> Self {
        WasmError::new(e).with_kind(WasmErrorKind::Validation)
    }
}

impl From<String> for WasmError {
    fn from(e: String) -> Self {
        WasmError::new(e)
    }
}

impl From<&str> for WasmError {
    fn from(e: &str) -> Self {
        WasmError::new(e)
    }
}

/// Convenience alias — mirrors the `SDKResult<T>` pattern in the base crate.
pub type WasmResult<T> = Result<T, WasmError>;
