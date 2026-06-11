//! Error types for the Trading SDK.

use std::string::FromUtf8Error;

use thiserror::Error;

use crate::generated::types::{ApiErrorDetail, ApiErrorResponse};

/// Render each variant for human consumption.
///
/// `JsonValidationErrorDetail` surfaces its `rule` + `message` inline — what
/// you actually want to see in a log line. The catch-all `Object` variant
/// is free-form upstream data we don't try to interpret, so it falls back
/// to a compact JSON dump.
impl std::fmt::Display for ApiErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonValidationErrorDetail(d) => write!(f, "{}: {}", d.rule, d.message),
            Self::Object(map) => match serde_json::to_string(map) {
                Ok(s) => f.write_str(&s),
                Err(_) => write!(f, "{map:?}"),
            },
        }
    }
}

impl std::fmt::Display for ApiErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}: {}", self.status, self.message)?;
        if let Some(details) = &self.details {
            write!(f, " ({details})")?;
        }
        if let Some(error_id) = &self.error_id {
            write!(f, " [error_id={error_id}]")?;
        }
        Ok(())
    }
}

impl ApiErrorResponse {
    /// Whether this error is potentially transient and the operation could
    /// be retried with backoff.
    pub fn is_retryable(&self) -> bool {
        self.status == 429 || self.status >= 500
    }

    /// Whether the HTTP status code was lost during error conversion.
    ///
    /// This happens when the server returns a non-JSON body (e.g. HTML from a
    /// load balancer) that progenitor can't deserialize. The raw body is
    /// preserved in `message`, but the status code is unavailable.
    ///
    /// Callers may want to treat these as retryable (usually transient proxy
    /// errors) but should be aware that a 4xx with a non-JSON body would also
    /// produce `status == 0`.
    pub fn is_status_unknown(&self) -> bool {
        self.status == 0
    }
}

/// Errors that can occur when using the Trading SDK.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum SDKError {
    /// Invalid network configuration.
    #[error("Invalid network connection specified")]
    InvalidNetwork,

    /// Invalid private key format or length.
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// JSON serialization error.
    #[error(transparent)]
    JsonSerializeError(#[from] serde_json::Error),

    /// HTTP client error.
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Structured API error from the trading API.
    ///
    /// Boxed because `ApiErrorResponse` carries optional details + error_id
    /// strings and a nested enum, making the variant large enough to bloat
    /// every `SDKResult<T>` on the stack (clippy::result_large_err). Boxing
    /// pushes the cost to error-construction time (negligible — happy paths
    /// don't allocate) and keeps Result types cheap to pass around.
    #[error("API error: {0}")]
    ApiError(Box<ApiErrorResponse>),

    /// Client-side request error (not from the server).
    #[error("Request error: {0}")]
    RequestError(String),

    /// No keypair available for signing.
    #[error(
        "No keypair available. Provide a signer via Transaction::builder().signer() or Client::builder().keypair()"
    )]
    MissingKeypair,

    #[error(transparent)]
    StringParseError(#[from] FromUtf8Error),

    #[error("Failed to read chain_id {0}")]
    ChainIdCastError(std::num::TryFromIntError),

    #[error("Provided URL was neither websocket or rest url")]
    InvalidNetworkUrl,

    #[error("Invalid schema response: missing or invalid '{0}' field")]
    InvalidSchemaResponse(&'static str),

    #[error("Invalid chain hash: {0}")]
    InvalidChainHash(String),

    #[error("Transaction serialization failed: {0}")]
    SerializationError(String),

    #[error("System time error: clock is before UNIX epoch")]
    SystemTimeError,

    #[error("Invalid signature length: expected 64 bytes, got {0}")]
    InvalidSignatureLength(usize),

    #[error("Invalid public key length: expected 32 bytes, got {0}")]
    InvalidPublicKeyLength(usize),

    #[error("Schema outdated - recompile the binary to update bullet-exchange-interface")]
    SchemaOutdated,

    #[error("CallMessage {0} must be added to user-actions")]
    UnsupportedCallMessage(String),

    #[error("Transaction is outdated - need to re-sign again.")]
    TransactionOutdated,

    #[error("Invalid multisig: {0}")]
    InvalidMultisig(String),

    /// Sub-account index out of range. The runtime tracks sub-account
    /// existence in a `u32` bitmask, so valid indices are `0..=31`. Carries the
    /// caller-supplied value as `u32` so an out-of-`u8` value (e.g. from a JS
    /// number) is reported faithfully rather than truncated.
    #[error("Sub-account index {0} out of range (0..=31)")]
    InvalidSubAccountIndex(u32),

    #[error(transparent)]
    WebsocketError(#[from] Box<WSErrors>),
}

impl From<WSErrors> for SDKError {
    fn from(err: WSErrors) -> Self {
        SDKError::WebsocketError(Box::new(err))
    }
}

#[derive(Debug, Error)]
pub enum WSErrors {
    // WebSocket errors
    /// WebSocket connection error.
    #[error("WebSocket connection error: {0}")]
    WsConnectionError(String),

    /// WebSocket upgrade error.
    #[error(transparent)]
    WsUpgradeError(#[from] reqwest_websocket::Error),

    /// WebSocket connection was closed by the server.
    #[error("WebSocket closed ({code}): {reason}")]
    WsClosed {
        /// Close code from the server
        code: reqwest_websocket::CloseCode,
        /// Close reason from the server
        reason: String,
    },

    /// WebSocket stream ended unexpectedly without a close frame.
    #[error("WebSocket stream ended unexpectedly")]
    WsStreamEnded,

    /// WebSocket connection handshake timed out.
    #[error("WebSocket connection timed out waiting for server")]
    WsConnectionTimeout,

    /// WebSocket server did not send expected connected message.
    #[error("Expected 'connected' status, got: {0}")]
    WsHandshakeFailed(String),

    /// WebSocket protocol error.
    #[error("WebSocket error: {0}")]
    WsError(String),

    /// WebSocket server returned an error.
    #[error("WebSocket server error (code {code}): {message}")]
    WsServerError { code: i32, message: String },

    /// JSON serialization error.
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

impl SDKError {
    /// Whether this error is potentially transient and the operation could
    /// be retried with backoff.
    pub fn is_retryable(&self) -> bool {
        match self {
            SDKError::HttpError(e) => e.is_timeout() || e.is_request(),
            SDKError::ApiError(resp) => resp.is_retryable(),
            SDKError::WebsocketError(e) => matches!(
                e.as_ref(),
                WSErrors::WsConnectionError(_)
                    | WSErrors::WsStreamEnded
                    | WSErrors::WsConnectionTimeout
            ),
            _ => false,
        }
    }

    /// If this is an API error, returns the structured response.
    pub fn api_error(&self) -> Option<&ApiErrorResponse> {
        match self {
            SDKError::ApiError(resp) => Some(resp.as_ref()),
            _ => None,
        }
    }
}

pub type SDKResult<T, E = SDKError> = Result<T, E>;

impl From<progenitor_client::Error<ApiErrorResponse>> for SDKError {
    fn from(err: progenitor_client::Error<ApiErrorResponse>) -> Self {
        match err {
            progenitor_client::Error::ErrorResponse(resp) => {
                SDKError::ApiError(Box::new(resp.into_inner()))
            }
            progenitor_client::Error::CommunicationError(e) => SDKError::HttpError(e),
            progenitor_client::Error::ResponseBodyError(e) => SDKError::HttpError(e),
            progenitor_client::Error::InvalidUpgrade(e) => SDKError::HttpError(e),
            // With 4XX/5XX ranges injected in build.rs, UnexpectedResponse only
            // fires for truly exotic status codes (1xx, 3xx). Body can't be read
            // synchronously so we only preserve the status code.
            progenitor_client::Error::UnexpectedResponse(resp) => {
                let status = resp.status().as_u16();
                SDKError::ApiError(Box::new(ApiErrorResponse {
                    status,
                    message: format!("HTTP {status}"),
                    details: None,
                    error_id: None,
                }))
            }
            // Server returned 4XX/5XX but the body couldn't be deserialized as
            // ApiErrorResponse (e.g., HTML from a load balancer, plain text, etc).
            // Progenitor doesn't preserve the status code on this variant so we
            // can't determine retryability. We surface the raw body as the message.
            progenitor_client::Error::InvalidResponsePayload(bytes, _) => {
                let body = String::from_utf8_lossy(&bytes);
                SDKError::ApiError(Box::new(ApiErrorResponse {
                    status: 0,
                    message: body.into_owned(),
                    details: None,
                    error_id: None,
                }))
            }
            // Client-side errors (InvalidRequest, PreHookError) that aren't HTTP
            // responses at all.
            other => SDKError::RequestError(format!("{other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    async fn mock_submit_tx(status: u16, body: serde_json::Value) -> (MockServer, SDKError) {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tx/submit"))
            .respond_with(ResponseTemplate::new(status).set_body_json(&body))
            .mount(&server)
            .await;

        let client = crate::generated::Client::new(&server.uri());
        let result = client
            .submit_tx(&crate::generated::types::SubmitTxRequest { body: "dGVzdA==".into() })
            .await;

        (server, result.unwrap_err().into())
    }

    #[tokio::test]
    async fn error_response_is_structured() {
        use crate::generated::types::ApiErrorDetail;

        let (_server, err) = mock_submit_tx(
            400,
            serde_json::json!({
                "status": 400,
                "message": "Transaction validation failed: insufficient funds",
                "details": {"reason": "insufficient_balance"}
            }),
        )
        .await;

        let resp = err.api_error().expect("should be ApiError");
        assert_eq!(resp.status, 400);
        assert_eq!(resp.message, "Transaction validation failed: insufficient funds");
        match resp.details.as_ref().expect("details present") {
            ApiErrorDetail::Object(map) => {
                assert_eq!(map["reason"], "insufficient_balance");
            }
            other => panic!("expected Object variant, got {other:?}"),
        }
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("insufficient funds"));
    }

    #[tokio::test]
    async fn error_response_5xx_is_retryable() {
        let (_server, err) = mock_submit_tx(
            503,
            serde_json::json!({
                "status": 503,
                "message": "Service unavailable"
            }),
        )
        .await;

        assert!(err.is_retryable());
        assert_eq!(err.api_error().unwrap().status, 503);
    }

    #[tokio::test]
    async fn error_response_malformed_body_preserves_raw_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tx/submit"))
            .respond_with(
                ResponseTemplate::new(502).set_body_string("<html><body>Bad Gateway</body></html>"),
            )
            .mount(&server)
            .await;

        let client = crate::generated::Client::new(&server.uri());
        let result = client
            .submit_tx(&crate::generated::types::SubmitTxRequest { body: "dGVzdA==".into() })
            .await;

        let err: SDKError = result.unwrap_err().into();
        // Malformed body still becomes an ApiError with the raw body as message.
        // Status code is lost (progenitor limitation) so status is 0.
        let resp = err.api_error().expect("should be ApiError");
        assert_eq!(resp.status, 0);
        assert!(resp.message.contains("Bad Gateway"));
        // status=0 means the status code was lost (progenitor limitation).
        // is_retryable() returns false since we can't be sure it's a 5xx.
        // Callers can use is_status_unknown() to decide for themselves.
        assert!(!err.is_retryable());
        assert!(resp.is_status_unknown());
    }

    #[tokio::test]
    async fn error_response_surfaces_error_id() {
        let (_server, err) = mock_submit_tx(
            400,
            serde_json::json!({
                "status": 400,
                "message": "Transaction validation failed",
                "error_id": "8b2e4d9f-7a1c-4f0e-9c5d-3e6a8b1c2d4f"
            }),
        )
        .await;

        let resp = err.api_error().expect("should be ApiError");
        assert_eq!(resp.error_id.as_deref(), Some("8b2e4d9f-7a1c-4f0e-9c5d-3e6a8b1c2d4f"));
        assert!(err.to_string().contains("8b2e4d9f-7a1c-4f0e-9c5d-3e6a8b1c2d4f"));
    }

    #[tokio::test]
    async fn error_response_parses_json_validation_detail() {
        use crate::generated::types::ApiErrorDetail;

        let (_server, err) = mock_submit_tx(
            400,
            serde_json::json!({
                "status": 400,
                "message": "Invalid request payload",
                "details": {
                    "rule": "wrong_type",
                    "message": "invalid type: integer, expected a string at line 4 column 30"
                }
            }),
        )
        .await;

        let resp = err.api_error().expect("should be ApiError");
        match resp.details.as_ref().expect("details present") {
            ApiErrorDetail::JsonValidationErrorDetail(d) => {
                assert_eq!(d.rule, "wrong_type");
                assert!(d.message.contains("expected a string"));
            }
            other => panic!("expected JsonValidationErrorDetail variant, got {other:?}"),
        }
    }
}
