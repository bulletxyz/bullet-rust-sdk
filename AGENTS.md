# AGENTS.md

Project-specific guidance for Claude Code on the bullet-rust-sdk.

## Project Structure

This repository contains two crates that must stay in sync:

- `rust/` - The core Rust SDK (`bullet-rust-sdk`)
- `wasm/` - WASM bindings for JavaScript/TypeScript (`bullet-rust-sdk-wasm`)

## WASM/Rust Synchronization

**CRITICAL:** The WASM bindings must mirror the Rust SDK.

When adding, editing, or deleting any of the following in the Rust crate, the corresponding changes **must** be made to the WASM crate:

- Public structs and their fields
- Public methods on structs
- Public functions
- Builder patterns and their methods
- Error variants

### Key File Mappings

| Rust (`rust/src/`)         | WASM (`wasm/src/`)              |
|----------------------------|----------------------------------|
| `client.rs`                | `client.rs`                      |
| `transaction_builder.rs`   | `transaction_builder.rs`         |
| `transactions.rs`          | `transactions.rs`                |
| `keypair.rs`               | `keypair.rs`                     |
| `errors.rs`                | `errors.rs`                      |
| `ws/client.rs`             | `ws/client.rs`                   |
| `ws/topics.rs`             | `ws/topics.rs`                   |

### WASM Error Handling

All fallible functions in the WASM crate **must** return `WasmResult<T>` (defined in `wasm/src/errors.rs`), never `Result<T, String>` or `Result<T, JsValue>`. `WasmError` has a blanket `From<E: Display>` impl so `?` works automatically with any error type.

### WASM Naming Conventions

- Rust `snake_case` methods become `camelCase` in WASM (via `#[wasm_bindgen(js_name = ...)]`)
- WASM wrapper structs are prefixed with `Wasm` (e.g., `WasmKeypair`, `WasmTradingApi`)
- The JS-facing name drops the prefix (e.g., `#[wasm_bindgen(js_name = Keypair)]`)

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

Generated code is written to `$OUT_DIR/call_message_factories.rs` and included via `include!()` in `wasm/src/transactions.rs`.

## Development Commands

Use `just` for all common development tasks. Run `just` to see available recipes.

### Testing

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
