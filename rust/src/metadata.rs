//! Exchange metadata: symbol lookups and market info.
//!
//! Metadata is fetched from the exchange during [`Client`] construction and
//! cached for the lifetime of the client. Use [`Client::refresh_metadata`] to
//! update it for long-running processes.
//!
//! # Example
//!
//! ```ignore
//! use bullet_rust_sdk::{Client, MarketId};
//!
//! let client = Client::mainnet().await?;
//!
//! // Resolve symbol string to numeric MarketId
//! let market_id = client.market_id("BTC-USD").expect("unknown symbol");
//!
//! // Get all symbols
//! for sym in client.symbols() {
//!     println!("{}: MarketId({})", sym.symbol, sym.market_id.0);
//! }
//! ```

use std::collections::HashMap;

use bullet_exchange_interface::types::MarketId;

use crate::generated::types::Symbol;

/// Cached exchange metadata for fast symbol lookups.
#[derive(Debug, Clone)]
pub(crate) struct ExchangeMetadata {
    symbols: Vec<SymbolInfo>,
    /// symbol string -> index into `symbols`
    by_name: HashMap<String, usize>,
    /// market_id.0 -> index into `symbols`
    by_id: HashMap<u16, usize>,
}

/// Symbol information cached from the exchange.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// Trading pair symbol (e.g. `"BTC-USD"`).
    pub symbol: String,
    /// Numeric market identifier.
    pub market_id: MarketId,
    /// Trading status (e.g. `"TRADING"`, `"HALT"`).
    pub status: String,
    /// Base asset (e.g. `"BTC"`).
    pub base_asset: String,
    /// Quote asset (e.g. `"USD"`).
    pub quote_asset: String,
    /// Price decimal precision.
    pub price_precision: u8,
    /// Quantity decimal precision.
    pub quantity_precision: u8,
}

impl ExchangeMetadata {
    pub(crate) fn from_symbols(raw: &[Symbol]) -> Self {
        let symbols: Vec<SymbolInfo> = raw
            .iter()
            .map(|s| SymbolInfo {
                symbol: s.symbol.clone(),
                market_id: MarketId(s.market_id),
                status: s.status.clone(),
                base_asset: s.base_asset.clone(),
                quote_asset: s.quote_asset.clone(),
                price_precision: s.price_precision,
                quantity_precision: s.quantity_precision,
            })
            .collect();

        let by_name = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.symbol.clone(), i))
            .collect();

        let by_id = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.market_id.0, i))
            .collect();

        Self {
            symbols,
            by_name,
            by_id,
        }
    }

    pub(crate) fn market_id(&self, symbol: &str) -> Option<MarketId> {
        self.by_name
            .get(symbol)
            .map(|&i| self.symbols[i].market_id)
    }

    pub(crate) fn symbol_info_by_name(&self, symbol: &str) -> Option<&SymbolInfo> {
        self.by_name.get(symbol).map(|&i| &self.symbols[i])
    }

    pub(crate) fn symbol_info_by_id(&self, market_id: MarketId) -> Option<&SymbolInfo> {
        self.by_id.get(&market_id.0).map(|&i| &self.symbols[i])
    }

    pub(crate) fn symbols(&self) -> &[SymbolInfo] {
        &self.symbols
    }
}
