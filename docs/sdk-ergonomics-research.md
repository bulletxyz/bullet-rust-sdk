# SDK Ergonomics Research: Bullet vs. the Field

**Date:** March 2026
**Scope:** Bullet Rust SDK vs. Hyperliquid (official), Ferrofluid (Hyperliquid community), and Binance Rust SDKs

---

## TL;DR

Bullet's SDK has genuinely strong bones — compile-time typestate builders, schema validation at startup, type-safe WebSocket topics, and unique WASM bindings. The core infrastructure is better-engineered than any of the Hyperliquid SDKs. The primary gap is **ergonomics at the trading layer**: users must manually construct deeply nested `CallMessage` enums from an external crate, WebSocket lacks auto-reconnect and per-topic channels, and two parallel transaction APIs exist simultaneously with subtle inconsistencies. Fixing these would make Bullet substantially better DX than anything in the ecosystem.

---

## SDKs Analysed

| SDK | Exchange | Version | Async | Stars |
|-----|----------|---------|-------|-------|
| **bullet-rust-sdk** | Bullet | 0.1.0 | ✅ tokio | — |
| **hyperliquid-rust-sdk** | Hyperliquid | 0.6.0 | ✅ tokio | ~350 |
| **ferrofluid** | Hyperliquid | 0.2.0 | ✅ tokio | ~120 |
| **binance-sdk** (official) | Binance | 44.0.1 | ✅ tokio | ~500 |
| **binance** (wisespace-io) | Binance | 0.21.2 | ❌ blocking | ~700 |

---

## What Bullet Does Better

### 1. Compile-time typestate builders (`bon`)

Bullet uses `bon`'s typestate builder for `Transaction`, meaning missing required fields are a **compile error**, not a runtime panic:

```rust
// Bullet — missing call_message = compile error
Transaction::builder()
    .max_fee(10_000_000)
    .signer(&keypair)
    .send(&client)       // ← won't compile: call_message is required
    .await?;
```

Contrast with Ferrofluid's `OrderBuilder`, which has no typestate — missing a price before `.send()` is a runtime error. The official Hyperliquid SDK has no builder at all; you fill a struct literal with all fields.

The Binance official SDK (also using a builder) *does* put required params in the constructor, which is close — but its builders aren't typestate-enforced across the full chain.

**Verdict:** Bullet wins clearly here. This is a meaningful safety property for financial code.

---

### 2. WASM bindings (unique)

No other SDK in this comparison offers first-class WebAssembly/JavaScript bindings. Bullet's WASM crate mirrors the Rust API surface via `wasm-bindgen` with build-time codegen for `CallMessage` factories. This opens the SDK to browser-based tooling, dashboards, and TypeScript trading bots — a significant differentiator.

---

### 3. Schema validation at startup

On `Client::new()`, Bullet fetches the remote schema, validates it against the compiled `bullet-exchange-interface` types, and extracts the chain hash. This catches API version mismatches before a single trade is attempted — something none of the other SDKs do.

```rust
// Fail fast: panics (see issue below) if server schema doesn't match binary
let client = Client::mainnet().await?;
```

The Hyperliquid SDK fetches `Meta` (asset mappings) on init, but doesn't validate anything structurally. Ferrofluid doesn't fetch metadata at all on construction.

---

### 4. Type-safe WebSocket topics

Bullet's `Topic` enum is the cleanest WS subscription API in this comparison:

```rust
// Bullet
ws.subscribe([
    Topic::agg_trade("BTC-USD"),
    Topic::depth("ETH-USD", OrderbookDepth::D10),  // depth is an enum, not "10"
    Topic::kline("BTC-USD", KlineInterval::H1),    // interval is an enum, not "1h"
], None).await?;
```

The Hyperliquid SDK uses:
```rust
// Hyperliquid — interval is a raw String, typos are runtime errors
info_client.subscribe(Subscription::Candle {
    coin: "ETH".to_string(),
    interval: "1h".to_string(),  // ← typo = silent failure
}, sender).await?;
```

Ferrofluid is the same — candle intervals are `String`. Bullet's enums (`KlineInterval::H1`, `OrderbookDepth::D10`) are strictly better.

---

### 5. Ergonomic network selection

```rust
// Bullet — all of these work
Client::mainnet().await?;
Client::builder().network(Network::Testnet).build().await?;
Client::builder().network("https://custom.example.com").build().await?;
```

The `From<&str>` impl on `Network` means a custom URL or a named network can be passed interchangeably. The official Binance SDK and Hyperliquid SDK have similar named-constructor patterns, but Bullet's `From<&str>` fallthrough to `Custom(url)` is the most flexible.

---

### 6. `Deref` for transparent generated-client access

Bullet wraps a progenitor-generated client and exposes all methods via `Deref`, so users don't need to call `.client()`:

```rust
// Direct access — no ceremony
let info = api.exchange_info().await?.into_inner();
let tickers = api.ticker_price(None).await?.into_inner();
```

The Hyperliquid SDK requires users to pick the right client type (`ExchangeClient` vs `InfoClient`) and keep both around.

---

### 7. `ServerMessage::Unknown` fallback on WebSocket

Bullet's `recv()` never returns an error on parse failure — it yields `ServerMessage::Unknown(error_str, raw_text)` instead. This means new server message types don't break existing binaries:

```rust
ServerMessage::Unknown(e, raw) => {
    // Log and continue — don't crash
    warn!(?e, "Unknown message: {raw}");
}
```

The Hyperliquid SDK propagates parse errors; Ferrofluid also has an `InvalidResponse` error path. Bullet's approach is better for production resilience.

---

### 8. Unified URL derivation

Bullet auto-derives the WebSocket URL from the REST URL (`https://` → `wss://`), so users never need to configure two URLs. All other SDKs require WebSocket URLs to be specified separately or use hardcoded constants.

---

## What Could Be Improved

### 1. No auto-reconnect on WebSocket ⚠️ (Highest Priority)

The WebSocket client docs literally say to wrap everything in `'reconnect: loop { ... continue 'reconnect; }`. This is boilerplate every user must write:

```rust
// Users have to write this every time
'reconnect: loop {
    let mut ws = api.connect_ws().call().await?;
    loop {
        match ws.recv().await {
            Err(WSErrors::WsClosed { .. }) => continue 'reconnect,
            Err(WSErrors::WsStreamEnded) => continue 'reconnect,
            // ...
        }
    }
}
```

**Ferrofluid's solution:** `ManagedWsProvider` with configurable backoff, reconnect, and automatic subscription replay after reconnect. The `Arc`-wrapped design lets it be shared across tasks trivially.

**Hyperliquid SDK's solution:** Background reader task auto-reconnects with 1s delay; users just keep receiving from their channels.

**Recommendation:** Add a `ManagedWebsocket` that wraps the raw handle, handles reconnection with exponential backoff, and replays subscriptions. Keep the raw `WebsocketHandle` for power users who need full control.

---

### 2. Raw `CallMessage` construction is verbose ⚠️ (Highest Priority for DX)

This is the biggest ergonomics gap. Users must construct trading operations like this:

```rust
use bullet_exchange_interface::message::{CallMessage, UserAction, NewOrderArgs};
use bullet_exchange_interface::types::{Side, OrderType, MarketId};

let call_msg = CallMessage::User(UserAction::PlaceOrders {
    market_id: MarketId(0),
    orders: vec![NewOrderArgs {
        price: 50000_u32.into(),
        size: 1_u8.into(),
        side: Side::Ask,
        order_type: OrderType::ImmediateOrCancel,
        reduce_only: false,
        client_order_id: None,
        pending_tpsl_pair: None,
    }],
    replace: false,
    sub_account_index: None,
}),
```

Compare to Ferrofluid:
```rust
exchange.order(0).limit_buy("50000", "0.001").send().await?;
```

Or Binance official SDK:
```rust
let params = NewOrderParams::builder("BNBUSDT", Buy, Limit)
    .quantity(0.001).price(30000.0).time_in_force(Gtc).build()?;
rest_client.new_order(params).await?;
```

The imports span two crates (`bullet_exchange_interface`, `bullet_rust_sdk`), and all fields — even fields with sensible defaults like `replace: false` and `sub_account_index: None` — must be spelled out.

**Recommendation:** Add a high-level `OrderBuilder` in the SDK crate that wraps `CallMessage` construction, accepting symbol/market IDs, prices (with proper decimal handling), sizes, and sides:

```rust
// What this could look like
Transaction::builder()
    .place_order(
        PlaceOrder::builder()
            .market_id(MarketId(0))
            .limit_sell("50000", "1.0")
            .ioc()
            .build()
    )
    .send(&client)
    .await?;
```

---

### 3. Two parallel transaction APIs with inconsistencies

There are two complete APIs for building and submitting transactions:

**Imperative:**
```rust
let unsigned = client.build_transaction(call_msg, 10_000_000)?;
let signed = client.sign_transaction(unsigned, &keypair)?;
client.submit_transaction(&signed).await?;
```

**Builder:**
```rust
Transaction::builder()
    .call_message(call_msg)
    .max_fee(10_000_000)
    .signer(&keypair)
    .send(&client)
    .await?;
```

Problems:
- `build_transaction()` hardcodes `gas_limit: None` and `priority_fee: PriorityFeeBips(0)`, while the builder supports both.
- `submit_transaction()` and `send_transaction()` are aliases for the same method — pick one name.
- The websocket example uses the imperative API; docs promote the builder. A user reading both will be confused.

**Recommendation:** Deprecate the imperative API (or at minimum add clear docs saying "prefer `Transaction::builder()`"). Fix `build_transaction()` to use client defaults for gas_limit and priority_fee, matching builder behaviour.

---

### 4. Single-channel WebSocket (no per-topic routing)

`ws.recv()` returns any message from any subscription; callers must pattern-match a large `ServerMessage` enum to route by topic:

```rust
loop {
    match ws.recv().await? {
        ServerMessage::AggTrade(t) if t.symbol == "BTC-USD" => { /* ... */ }
        ServerMessage::DepthUpdate(d) if d.symbol == "ETH-USD" => { /* ... */ }
        ServerMessage::OrderUpdate(o) => { /* ... */ }
        _ => {}
    }
}
```

When processing many streams in separate tasks, this requires either channels + a demuxing task, or everything in one loop. Both Ferrofluid and Hyperliquid's SDK give each subscription its own `mpsc::UnboundedReceiver<Message>`, making per-topic task fanout natural:

```rust
// Ferrofluid
let (_, mut btc_book) = ws.subscribe_l2_book("BTC").await?;
let (_, mut eth_trades) = ws.subscribe_trades("ETH").await?;

tokio::select! {
    Some(msg) = btc_book.recv() => { /* process BTC book */ }
    Some(msg) = eth_trades.recv() => { /* process ETH trades */ }
}
```

**Recommendation:** Either (a) add per-subscription channels as an option, or (b) add a typed `recv_agg_trade()` / `recv_depth()` filter API on `WebsocketHandle` that wraps pattern matching internally.

---

### 5. Schema validation uses `panic!` instead of returning an error

In `client.rs:140`:
```rust
if left != right {
    panic!("Schema outdated - recompile the binary to update bullet-exchange-interface.")
}
```

A panic inside an `async fn` that returns `SDKResult<Self>` is surprising. Production servers often have panic handlers that log+exit, making this hard to distinguish from a real bug. It should be:

```rust
if left != right {
    return Err(SDKError::SchemaOutdated);
}
```

**Recommendation:** Add `SDKError::SchemaOutdated` variant, return it instead of panicking.

---

### 6. `.call()` on WebSocket builder is non-obvious

```rust
let mut ws = api.connect_ws().call().await?;
```

The `.call()` suffix is a `bon` artifact — when `#[builder]` is applied to an `async fn`, the generated builder uses `.call().await` instead of `.await` directly. This is not idiomatic and is confusing for users who don't know the `bon` internals.

**Recommendation:** Rename the builder finish method to `.connect()`:
```rust
let mut ws = api.connect_ws().connect().await?;
// or with config:
let mut ws = api.connect_ws().config(cfg).connect().await?;
```

This also makes the intent explicit in documentation and autocomplete.

---

### 7. WebSocket order placement requires manual Base64 encoding

```rust
// Users must know this ceremony:
let signed = client.sign_transaction(unsigned_tx, &keypair)?;
ws.order_place(Client::sign_to_base64(&signed)?, req_id).await?;
```

`ws.order_place` accepts `impl Into<String>`, expecting Base64-encoded Borsh bytes. This is an implementation detail that leaks into user code. Compare to Hyperliquid where signing and submission are fully opaque.

**Recommendation:** Add `ws.order_place_signed(&signed_tx, id)` that accepts `&SignedTransaction` directly and handles the serialization internally. Keep the string variant for advanced use cases.

---

### 8. `into_inner()` noise on every REST call

The progenitor-generated client returns `ResponseValue<T>`, requiring `.into_inner()` on every call:

```rust
let info = api.exchange_info().await?.into_inner();
let tickers = api.ticker_price(None).await?.into_inner();
let book = api.order_book(Some(5), &symbol).await?.into_inner();
```

This pattern appears in every example and is pure noise for users who don't need the response metadata.

**Recommendation:** Add thin wrapper methods on `Client` for common endpoints that unwrap `ResponseValue` automatically:

```rust
impl Client {
    pub async fn exchange_info_typed(&self) -> SDKResult<ExchangeInfo> {
        Ok(self.exchange_info().await?.into_inner())
    }
}
```

Or, document `.into_inner()` prominently and consider whether a macro can reduce it.

---

### 9. Prices and sizes lack decimal guidance

Neither `NewOrderArgs` nor the SDK documentation explains price/size representation. The types use `PositiveDecimal` from `bullet-exchange-interface`, but:
- `120_u8.into()` is used in examples — no guidance on what unit this is
- No `rust_decimal::Decimal` usage at the SDK boundary (unlike `binance-async`)
- Users coming from Binance (which uses string prices) or Hyperliquid (which uses f64) will be uncertain

**Recommendation:** Document price/size units explicitly in `NewOrderArgs` and the SDK docs. Consider a decimal formatting helper similar to Hyperliquid's `float_to_string_for_hashing()`.

---

### 10. `Keypair` cannot be exported after generation

```rust
let keypair = Keypair::generate(); // key is gone when process exits
```

There's no `to_hex()`, `secret_key_bytes()`, or any way to persist a generated keypair. The websocket example generates a throwaway keypair — which would never work on mainnet. This is both a usability gap and a footgun: users might not notice their key isn't being persisted.

**Recommendation:** Add `Keypair::secret_key_hex() -> String` and/or `Keypair::secret_key_bytes() -> [u8; 32]`, with a `Security Note` in the docs about storing private keys safely.

---

### 11. `ApiError(String)` loses structured error information

```rust
impl<T: std::fmt::Debug> From<progenitor_client::Error<T>> for SDKError {
    fn from(err: progenitor_client::Error<T>) -> Self {
        SDKError::ApiError(format!("{err:?}"))
    }
}
```

Progenitor errors contain structured data (status code, response body), but this `From` impl converts everything to a debug string. Callers can't match on `status_code == 429` vs `status_code == 400` without parsing the string.

**Recommendation:** Add `SDKError::HttpError { status_code: u16, body: String }` and extract the structured info from the progenitor error before stringifying.

---

## Feature Gap Summary

| Feature | Bullet | Hyperliquid SDK | Ferrofluid | Binance official |
|---------|--------|-----------------|------------|-----------------|
| Typestate builder | ✅ (`bon`) | ❌ struct literal | ❌ no typestate | ✅ (partial) |
| WASM bindings | ✅ | ❌ | ❌ | ❌ |
| Schema validation on init | ✅ | ❌ | ❌ | ❌ |
| Type-safe WS topics | ✅ enums | ⚠️ strings for interval | ⚠️ strings for interval | ✅ enums |
| Auto-reconnect WS | ❌ | ✅ background task | ✅ `ManagedWsProvider` | ❌ |
| Per-topic WS channels | ❌ | ✅ | ✅ | N/A |
| High-level order builder | ❌ | ⚠️ struct literal | ✅ `OrderBuilder` | ✅ |
| Order tracking | ❌ | ❌ | ✅ opt-in | ❌ |
| Typed error structure | ⚠️ partial | ⚠️ two-layer | ✅ | ⚠️ `anyhow` |
| Rate limiting | ❌ | ❌ | ⚠️ error only | ❌ |
| Private key export | ❌ | ✅ (alloy) | ✅ (alloy) | ✅ |
| Decimal price types | ❌ | ❌ (`f64`) | ❌ (`String`) | ✅ (`rust_decimal`) |

---

## Recommended Priorities

Ordered by trader DX impact:

### P0 — Critical for production traders
1. **Auto-reconnect WebSocket** (`ManagedWebsocket` / `connect_ws_managed()`)
   Every production trader needs this. Writing reconnect loops manually is error-prone.

2. **High-level order builder** (`PlaceOrderBuilder` or similar)
   The raw `CallMessage::User(UserAction::PlaceOrders { ... })` construction is the single biggest friction point. A fluent wrapper that knows about `MarketId`, side, price, size, and TIF is table stakes.

### P1 — Important for developer experience
3. **Per-topic WebSocket channels** (demux `recv()` or expose per-subscription receivers)
4. **Consolidate transaction APIs** (make `Transaction::builder()` the single path; remove or clearly deprecate imperative API)
5. **Schema panic → `SDKError::SchemaOutdated`**
6. **`Keypair::secret_key_hex()` / `secret_key_bytes()`**

### P2 — Polish
7. **Rename `connect_ws().call()` → `connect_ws().connect()`**
8. **`ws.order_place_signed(&SignedTransaction, id)`**
9. **Reduce `into_inner()` noise** via typed wrapper methods
10. **Document price/size units** in `NewOrderArgs` and add decimal helpers
11. **Structured `ApiError` variant** (status_code + body, not a debug string)

---

## Conclusion

Bullet's SDK is architecturally ahead of the Hyperliquid ecosystem on compile-time safety (typestate builders), startup validation, and WASM support. The foundations are solid. The gap is in the **trading-layer ergonomics**: constructing orders, managing WebSocket connections over time, and reducing the surface area of types users need to import from external crates.

The two highest-leverage improvements — auto-reconnecting WebSocket and a high-level order builder — would leapfrog every Rust trading SDK in the space and give Bullet the best developer experience of any permissionless exchange SDK in Rust.
