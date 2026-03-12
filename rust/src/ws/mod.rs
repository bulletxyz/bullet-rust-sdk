pub mod client;
pub mod models;
pub mod topics;

// Re-export commonly used types at ws module level
pub use client::{WebsocketConfig, WebsocketHandle};
pub use models::{ServerMessage, TaggedMessage};
pub use topics::{KlineInterval, OrderbookDepth, Topic};
