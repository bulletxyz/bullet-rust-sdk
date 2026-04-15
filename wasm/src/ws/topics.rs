use bullet_rust_sdk::ws::topics::{
    KlineInterval as SdkKlineInterval, OrderbookDepth as SdkDepth, Topic as SdkTopic,
};
use wasm_bindgen::prelude::*;

/// Orderbook depth level for WebSocket depth subscriptions.
/// @enum {number}
#[wasm_bindgen(js_name = OrderbookDepth)]
pub enum WasmOrderbookDepth {
    /// 5 levels
    D5,
    /// 10 levels
    D10,
    /// 20 levels
    D20,
}

/// Kline/candlestick interval for WebSocket kline subscriptions.
/// @enum {number}
#[wasm_bindgen(js_name = KlineInterval)]
pub enum WasmKlineInterval {
    /// 1 minute
    M1,
    /// 5 minutes
    M5,
    /// 15 minutes
    M15,
    /// 30 minutes
    M30,
    /// 1 hour
    H1,
    /// 4 hours
    H4,
    /// 1 day
    D1,
}

/// A typed WebSocket subscription topic.
///
/// Build with the static factory methods, then pass to
/// `WebsocketHandle.subscribe()`.
///
/// @example
/// ```js
/// const topics = [Topic.aggTrade("BTC-USD"), Topic.depth("ETH-USD", OrderbookDepth.D10)];
/// await ws.subscribe(topics);
/// ```
#[wasm_bindgen(js_name = Topic)]
pub struct WasmTopic {
    inner: String,
}

#[wasm_bindgen(js_class = Topic)]
impl WasmTopic {
    /// Create an aggregate trade topic, e.g. `"BTC-USD@aggTrade"`.
    /// @param {string} symbol - The market symbol.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = aggTrade)]
    pub fn agg_trade(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::agg_trade(symbol).to_string(),
        }
    }

    /// Create an orderbook depth topic, e.g. `"BTC-USD@depth10"`.
    /// @param {string} symbol - The market symbol.
    /// @param {OrderbookDepth} depth - Number of price levels.
    /// @returns {Topic}
    pub fn depth(symbol: &str, depth: WasmOrderbookDepth) -> WasmTopic {
        let d = match depth {
            WasmOrderbookDepth::D5 => SdkDepth::D5,
            WasmOrderbookDepth::D10 => SdkDepth::D10,
            WasmOrderbookDepth::D20 => SdkDepth::D20,
        };
        WasmTopic {
            inner: SdkTopic::depth(symbol, d).to_string(),
        }
    }

    /// Create a book ticker topic, e.g. `"BTC-USD@bookTicker"`.
    /// @param {string} symbol - The market symbol.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = bookTicker)]
    pub fn book_ticker(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::book_ticker(symbol).to_string(),
        }
    }

    /// Create a mark price topic, e.g. `"BTC-USD@markPrice"`.
    /// @param {string} symbol - The market symbol.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = markPrice)]
    pub fn mark_price(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::mark_price(symbol).to_string(),
        }
    }

    /// Create a kline/candlestick topic, e.g. `"BTC-USD@kline_1h"`.
    /// @param {string} symbol - The market symbol.
    /// @param {KlineInterval} interval - The candlestick interval.
    /// @returns {Topic}
    pub fn kline(symbol: &str, interval: WasmKlineInterval) -> WasmTopic {
        let i = match interval {
            WasmKlineInterval::M1 => SdkKlineInterval::M1,
            WasmKlineInterval::M5 => SdkKlineInterval::M5,
            WasmKlineInterval::M15 => SdkKlineInterval::M15,
            WasmKlineInterval::M30 => SdkKlineInterval::M30,
            WasmKlineInterval::H1 => SdkKlineInterval::H1,
            WasmKlineInterval::H4 => SdkKlineInterval::H4,
            WasmKlineInterval::D1 => SdkKlineInterval::D1,
        };
        WasmTopic {
            inner: SdkTopic::kline(symbol, i).to_string(),
        }
    }

    /// Create a force order / liquidation topic, e.g. `"BTC-USD@forceOrder"`.
    /// @param {string} symbol - The market symbol.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = forceOrder)]
    pub fn force_order(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::force_order(symbol).to_string(),
        }
    }

    /// Subscribe to all market tickers.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = allTickers)]
    pub fn all_tickers() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_tickers().to_string(),
        }
    }

    /// Subscribe to all mark prices.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = allMarkPrices)]
    pub fn all_mark_prices() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_mark_prices().to_string(),
        }
    }

    /// Subscribe to all book tickers.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = allBookTickers)]
    pub fn all_book_tickers() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_book_tickers().to_string(),
        }
    }

    /// Subscribe to all force orders / liquidations.
    /// @returns {Topic}
    #[wasm_bindgen(js_name = allForceOrders)]
    pub fn all_force_orders() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_force_orders().to_string(),
        }
    }

    /// Wire-format string, e.g. `"BTC-USD@depth10"`.
    /// @returns {string}
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.inner.clone()
    }
}
