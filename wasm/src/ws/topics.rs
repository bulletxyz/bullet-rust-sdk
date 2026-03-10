use bullet_rust_sdk::ws::topics::{
    KlineInterval as SdkKlineInterval, OrderbookDepth as SdkDepth, Topic as SdkTopic,
};
use wasm_bindgen::prelude::*;

/// Orderbook depth level for WebSocket depth subscriptions.
#[wasm_bindgen(js_name = OrderbookDepth)]
pub enum WasmOrderbookDepth {
    D5,
    D10,
    D20,
}

/// Kline/candlestick interval for WebSocket kline subscriptions.
#[wasm_bindgen(js_name = KlineInterval)]
pub enum WasmKlineInterval {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
}

/// A typed WebSocket subscription topic.
///
/// Build with the static factory methods, then pass `toString()` to
/// `WasmWebsocketHandle.subscribe()`.
#[wasm_bindgen(js_name = Topic)]
pub struct WasmTopic {
    inner: String,
}

#[wasm_bindgen(js_class = Topic)]
impl WasmTopic {
    /// e.g. `"BTC-USD@aggTrade"`
    #[wasm_bindgen(js_name = aggTrade)]
    pub fn agg_trade(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::agg_trade(symbol).to_string(),
        }
    }

    /// e.g. `"BTC-USD@depth10"`
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

    /// e.g. `"BTC-USD@bookTicker"`
    #[wasm_bindgen(js_name = bookTicker)]
    pub fn book_ticker(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::book_ticker(symbol).to_string(),
        }
    }

    /// e.g. `"BTC-USD@markPrice"`
    #[wasm_bindgen(js_name = markPrice)]
    pub fn mark_price(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::mark_price(symbol).to_string(),
        }
    }

    /// e.g. `"BTC-USD@kline_1h"`
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

    /// e.g. `"BTC-USD@forceOrder"`
    #[wasm_bindgen(js_name = forceOrder)]
    pub fn force_order(symbol: &str) -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::force_order(symbol).to_string(),
        }
    }

    #[wasm_bindgen(js_name = allTickers)]
    pub fn all_tickers() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_tickers().to_string(),
        }
    }

    #[wasm_bindgen(js_name = allMarkPrices)]
    pub fn all_mark_prices() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_mark_prices().to_string(),
        }
    }

    #[wasm_bindgen(js_name = allBookTickers)]
    pub fn all_book_tickers() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_book_tickers().to_string(),
        }
    }

    #[wasm_bindgen(js_name = allForceOrders)]
    pub fn all_force_orders() -> WasmTopic {
        WasmTopic {
            inner: SdkTopic::all_force_orders().to_string(),
        }
    }

    /// Wire-format string, e.g. `"BTC-USD@depth10"`.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.inner.clone()
    }
}
