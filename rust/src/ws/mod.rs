pub mod client;
#[cfg(not(target_arch = "wasm32"))]
pub mod managed;
pub mod models;
pub mod topics;

// Re-export commonly used types at ws module level
pub use client::{WebsocketConfig, WebsocketHandle};
#[cfg(not(target_arch = "wasm32"))]
pub use managed::{ManagedWebsocket, ManagedWsConfig, WsEvent};
pub use models::{ServerMessage, TaggedMessage};
pub use topics::{KlineInterval, OrderbookDepth, Topic};
