//! Error types for the Trading SDK.

use std::string::FromUtf8Error;

use thiserror::Error;

/// Errors that can occur when using the Trading SDK.
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

    /// Progenitor client error.
    #[error("API error: {0}")]
    ApiError(String),

    /// No signer configured.
    #[error("No signer configured. Call .with_signer() before signing transactions.")]
    NoSigner,

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

pub type SDKResult<T, E = SDKError> = Result<T, E>;

impl<T: std::fmt::Debug> From<progenitor_client::Error<T>> for SDKError {
    fn from(err: progenitor_client::Error<T>) -> Self {
        SDKError::ApiError(format!("{err:?}"))
    }
}
