pub mod client;
pub mod managed;
pub mod models;
pub mod topics;

// Re-export commonly used types at ws module level
pub use client::{WebsocketConfig, WebsocketHandle};
pub use managed::{ManagedWebsocket, ManagedWsConfig, ManagedWsError, WsEvent};
pub use models::{ServerMessage, TaggedMessage};
pub use topics::{KlineInterval, OrderbookDepth, Topic};
