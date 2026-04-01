# bullet-rust-sdk-wasm

WebAssembly bindings for the Bullet trading SDK, built with `wasm-bindgen`.

## Structure

```
wasm/
├── build.rs                 # Codegen entry point
├── codegen/
│   ├── bullet_schema/       # CallMessage factory codegen (from bullet-exchange-interface)
│   └── progenitor/          # REST API type & client codegen (from progenitor output)
│       ├── walk/            # Parse progenitor-generated Rust with syn
│       └── emit/            # Emit wasm-bindgen wrappers
└── src/
    ├── client.rs            # WasmTradingApi — constructors and metadata
    ├── transaction_builder.rs # CallMessage factories, transaction building, signing
    ├── keypair.rs           # WasmKeypair
    ├── errors.rs            # WasmError / WasmResult
    ├── generated/           # Auto-generated wasm-bindgen wrappers (do not edit)
    │   └── mod.rs           # Includes progenitor_wrappers.rs from OUT_DIR
    └── ws/
        ├── client.rs        # WasmWebsocketHandle
        └── topics.rs        # WasmTopic, WasmOrderbookDepth, WasmKlineInterval
```

## Building

```bash
just build-wasm
# or directly:
wasm-pack build wasm --target web
```

Output is emitted to `wasm/pkg/`.

## Codegen Architecture

The WASM crate uses build-time codegen to generate wasm-bindgen wrappers for:

1. **REST API types** — Structs and enums from progenitor-generated code
2. **REST API client methods** — Async methods on `WasmTradingApi`
3. **CallMessage factories** — Transaction building helpers from bullet-exchange-interface

### How It Works

1. The **rust crate** (`bullet-rust-sdk`) runs progenitor at build time, generating REST client types and methods. It exposes the generated code path via Cargo `links` metadata.

2. The **wasm crate** (`bullet-rust-sdk-wasm`) reads that generated code at build time, parses it with `syn`, and emits wasm-bindgen wrapper types and client method implementations.

This approach ensures the WASM bindings stay synchronized with the Rust SDK automatically.

## Design Decision: Why Not Derive Macros?

We considered using `#[derive(WasmType)]` proc-macros on progenitor-generated types instead of build.rs codegen. This was rejected for the following reasons:

### 1. Duplicate Progenitor Generation

For derive macros to work in the wasm crate, progenitor would need to run again in the wasm crate's build.rs to generate types there. This means:
- Duplicate code generation (same types generated twice)
- Potential drift if configurations differ
- Wasted compile time

The current approach reuses the rust crate's progenitor output directly.

### 2. Crate Contamination

Derive macros expand where they're applied. If we applied `#[derive(WasmType)]` in the rust crate, wasm-bindgen code would be generated there, contaminating the pure Rust SDK with wasm concerns.

Each crate should be self-contained:
- **rust crate** — Pure Rust SDK, no wasm-bindgen dependencies
- **wasm crate** — All wasm-bindgen bindings and JS interop

### 3. Split Codegen Approaches

Derive macros only work on type definitions (structs, enums). Client method wrappers (`impl WasmTradingApi { ... }`) would still require build.rs codegen since they're not type definitions.

This would result in two different codegen strategies:
- Derive macros for types
- build.rs for client methods

The current unified build.rs approach is simpler to maintain and reason about.

## Adding New Endpoints

When the OpenAPI spec changes:

1. Rebuild the rust crate (progenitor regenerates types/client)
2. Rebuild the wasm crate (codegen picks up changes automatically)

No manual wrapper code needed — the codegen handles type mapping, getters, and client method wrappers.
