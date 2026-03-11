# bullet-rust-sdk-wasm

WebAssembly bindings for the Bullet trading SDK, built with `wasm-bindgen`.

## Structure

```
wasm/src/
├── client.rs              # WasmTradingApi — constructors and metadata
├── transactions.rs        # buildSignedTransaction, submitTransaction
├── keypair.rs             # WasmKeypair
├── errors.rs              # WasmError / WasmResult
├── generated/
│   ├── client.rs          # REST endpoint wrappers (impl WasmTradingApi)
│   ├── account.rs         # Account, Balance, AccountPosition, AccountAsset
│   ├── borrow.rs          # BorrowLendPoolResponse, InsuranceBalance, InsuranceAsset
│   ├── common.rs          # RollupConstants, ChainInfo, RateLimit, ModuleRef, …
│   ├── market.rs          # ExchangeInfo, OrderBook, Trade, Ticker24hr, …
│   ├── orders.rs          # BinanceOrder, LeverageBracket, Bracket
│   └── tx.rs              # SubmitTxResponse, TxReceipt, TxResult, …
└── ws/
    ├── client.rs          # WasmWebsocketHandle
    └── topics.rs          # WasmTopic, WasmOrderbookDepth, WasmKlineInterval
```

## Building

```bash
just build-wasm
# or directly:
wasm-pack build wasm --target web
```

Output is emitted to `wasm/pkg/`.

## TODO

- [ ] **Derive macro for `WasmWrapper` types** — today every type in
  `wasm/src/generated/{account,borrow,common,market,orders,tx}.rs` is a
  hand-written newtype wrapper with manually written `#[wasm_bindgen(getter)]`
  methods. Create a `#[derive(WasmWrapper)]` proc-macro that reads the field
  names and types of a `bullet_rust_sdk::codegen::types::*` struct and
  automatically emits:
  - the `#[wasm_bindgen(js_name = Foo)] pub struct WasmFoo(pub(crate) sdk::Foo);`
    newtype declaration
  - a `toJSON() -> String` method (via `serde_json::to_string`)
  - `#[wasm_bindgen(getter)]` methods for every field, mapping Rust types to
    wasm-bindgen-compatible ones (`i64`/`u64` → `f64`, `String` clone, `Option`
    passthrough, nested types wrapped recursively)

- [ ] **Code-generate `wasm/src/generated/client.rs` from the OpenAPI spec** —
  today the REST endpoint wrappers in `generated/client.rs` are written by hand
  and must be kept in sync with the progenitor-generated `Client` by convention.
  Instead, add a step (either in `build.rs` or as a standalone codegen binary)
  that:
  1. Reads `openapi.json` (the same spec consumed by `build.rs`)
  2. For each operation, determines the success response type and maps it to the
     corresponding `Wasm*` wrapper (or `Vec<Wasm*>` for array responses)
  3. Emits the `impl WasmTradingApi` block with correctly typed return values,
     replacing the current hand-maintained file
  This would make adding a new endpoint a zero-touch operation — rerun codegen
  and both the Rust SDK client and the WASM bindings update together.
