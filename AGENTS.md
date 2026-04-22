# AGENTS.md

Project-specific guidance for Claude Code on the bullet-rust-sdk.

## Project Structure

This repository contains two crates that must stay in sync:

- `rust/` - The core Rust SDK (`bullet-rust-sdk`)
- `wasm/` - WASM bindings for JavaScript/TypeScript (`bullet-rust-sdk-wasm`)

## WASM/Rust Synchronization

**CRITICAL:** The WASM bindings must mirror the Rust SDK. **Every change to one crate must be applied to both crates in the same pass.** Do not update Rust and leave WASM for later — they ship as one unit. If you add, edit, or delete a public API in one crate, make the corresponding change in the other crate before moving on.

This applies to:

- Public structs and their fields
- Public methods on structs
- Public functions
- Builder patterns and their methods
- Error variants

### README Documentation

When the WASM SDK's public API surface changes (new methods, renamed methods, removed methods, new types), update `wasm/README.md` to reflect the change. The README is published to npm and is the primary documentation for JS/TS users.

### Key File Mappings

| Rust (`rust/src/`)         | WASM (`wasm/src/`)              |
|----------------------------|----------------------------------|
| `client.rs`                | `client.rs`                      |
| `transaction_builder.rs`   | `transaction_builder.rs`         |
| `keypair.rs`               | `keypair.rs`                     |
| `errors.rs`                | `errors.rs`                      |
| `ws/client.rs`             | `ws/client.rs`                   |
| `ws/managed.rs`            | `ws/client.rs` (WasmManagedWebsocket) |
| `ws/topics.rs`             | `ws/topics.rs`                   |

### WASM Error Handling

All fallible functions in the WASM crate **must** return `WasmResult<T>` (defined in `wasm/src/errors.rs`), never `Result<T, String>` or `Result<T, JsValue>`. `WasmError` has a blanket `From<E: Display>` impl so `?` works automatically with any error type.

### WASM Naming Conventions

- Rust `snake_case` methods become `camelCase` in WASM (via `#[wasm_bindgen(js_name = ...)]`)
- WASM wrapper structs are prefixed with `Wasm` (e.g., `WasmKeypair`, `WasmTradingApi`)
- The JS-facing name drops the prefix (e.g., `#[wasm_bindgen(js_name = Keypair)]`)

### WASM JSDoc Annotations

All public WASM methods and constructors **must** have JSDoc annotations in their `///` doc comments. When you add or change a public method, update its JSDoc. Use `@param`, `@returns`, and `@example` tags with proper TypeScript types:

```rust
/// Create a depth topic.
/// @param {string} symbol - The market symbol.
/// @param {OrderbookDepth} depth - Number of price levels.
/// @returns {Topic}
pub fn depth(symbol: &str, depth: WasmOrderbookDepth) -> WasmTopic { ... }
```

- `@param {Type} name - Description.` for every parameter (use `[name]` for optional params)
- `@returns {Type}` for the return value (use `Promise<T>` for async methods)
- `@example` with a JS code block where usage isn't obvious
- Reference the JS-facing type names (e.g. `Topic`, not `WasmTopic`)

### Builder Pattern Notes

The Rust SDK uses `bon` for builders which creates type-state patterns. In WASM, use the `maybe_` variants to handle optional fields:

```rust
// WASM builder pattern
let inner = Client::builder()
    .network(network)
    .maybe_keypair(keypair)
    .maybe_max_fee(max_fee)
    .build()
    .await?;
```

### WASM Codegen for CallMessage

The WASM crate uses build-time codegen (`wasm/build.rs`) to generate `wasm_bindgen` factory methods for `CallMessage` variants. This walks the `bullet-exchange-interface` schema and generates:

- **Namespace structs** (`User`, `Public`, `Keeper`, `Vault`, `Admin`) with factory methods
- **Wrapper structs** for complex types (e.g., `WasmNewOrderArgs`)
- **Enum wrappers** for schema enums

The codegen lives in `wasm/codegen/` with two phases:
- `walk/` - Traverses the schema and extracts type information
- `emit/` - Generates Rust source code from the resolved data

Generated code is written to `$OUT_DIR/call_message_factories.rs` and included via `include!()` in `wasm/src/transaction_builder.rs`.

## Development Commands

Use `just` for all common development tasks. Run `just` to see available recipes.

### Testing

The project uses [cargo-nextest](https://nexte.st/) for Rust tests. Install once:

```bash
cargo install cargo-nextest --locked
```

A `.cargo/config.toml` alias routes `cargo t` through nextest. Prefer `just test` or `cargo t`; `cargo test --doc` is still used for doctests (nextest doesn't run them).

```bash
# Run Rust unit tests
just test

# Run doc tests
just test-doc

# Run integration tests (requires API endpoint)
just test-integration

# Run WASM Jest tests (requires build-wasm-node first)
just test-wasm

# Run all tests (unit + doc + WASM)
just test-all
```

### Building

```bash
# Check compilation
just check

# Build WASM for web and Node.js
just build-wasm
```

### Linting

```bash
# Run clippy
just lint

# Format code
just fmt
```
