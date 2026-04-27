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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::types::Symbol;

    fn mock_symbols() -> Vec<Symbol> {
        vec![
            Symbol {
                symbol: "BTC-USD".into(),
                market_id: 0,
                status: "TRADING".into(),
                base_asset: "BTC".into(),
                quote_asset: "USD".into(),
                price_precision: 2,
                quantity_precision: 3,
                pair: "BTCUSD".into(),
                contract_type: "PERPETUAL".into(),
                delivery_date: 0,
                onboard_date: 0,
                margin_asset: "USD".into(),
                base_asset_precision: 8,
                quote_precision: 8,
                underlying_type: "COIN".into(),
                underlying_sub_type: vec![],
                settle_plan: 0,
                trigger_protect: Default::default(),
                filters: vec![],
                order_types: vec![],
                time_in_force: vec![],
                liquidation_fee: Default::default(),
                market_take_bound: Default::default(),
                maker_fee_bps: vec![],
                taker_fee_bps: vec![],
            },
            Symbol {
                symbol: "ETH-USD".into(),
                market_id: 1,
                status: "TRADING".into(),
                base_asset: "ETH".into(),
                quote_asset: "USD".into(),
                price_precision: 2,
                quantity_precision: 4,
                pair: "ETHUSD".into(),
                contract_type: "PERPETUAL".into(),
                delivery_date: 0,
                onboard_date: 0,
                margin_asset: "USD".into(),
                base_asset_precision: 8,
                quote_precision: 8,
                underlying_type: "COIN".into(),
                underlying_sub_type: vec![],
                settle_plan: 0,
                trigger_protect: Default::default(),
                filters: vec![],
                order_types: vec![],
                time_in_force: vec![],
                liquidation_fee: Default::default(),
                market_take_bound: Default::default(),
                maker_fee_bps: vec![],
                taker_fee_bps: vec![],
            },
        ]
    }

    #[test]
    fn market_id_lookup() {
        let meta = ExchangeMetadata::from_symbols(&mock_symbols());
        assert_eq!(meta.market_id("BTC-USD"), Some(MarketId(0)));
        assert_eq!(meta.market_id("ETH-USD"), Some(MarketId(1)));
        assert_eq!(meta.market_id("SOL-USD"), None);
    }

    #[test]
    fn symbol_info_by_name_lookup() {
        let meta = ExchangeMetadata::from_symbols(&mock_symbols());
        let info = meta.symbol_info_by_name("ETH-USD").unwrap();
        assert_eq!(info.base_asset, "ETH");
        assert_eq!(info.quantity_precision, 4);
    }

    #[test]
    fn symbol_info_by_id_lookup() {
        let meta = ExchangeMetadata::from_symbols(&mock_symbols());
        let info = meta.symbol_info_by_id(MarketId(0)).unwrap();
        assert_eq!(info.symbol, "BTC-USD");
        assert!(meta.symbol_info_by_id(MarketId(99)).is_none());
    }

    #[test]
    fn symbols_returns_all() {
        let meta = ExchangeMetadata::from_symbols(&mock_symbols());
        assert_eq!(meta.symbols().len(), 2);
    }

    #[test]
    fn empty_symbols() {
        let meta = ExchangeMetadata::from_symbols(&[]);
        assert!(meta.symbols().is_empty());
        assert_eq!(meta.market_id("BTC-USD"), None);
    }
}
