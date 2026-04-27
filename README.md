# Bullet Rust SDK

A Rust SDK for interacting with the Bullet trading API, with WASM bindings for JavaScript/TypeScript.

## Project Structure

```
bullet-rust-sdk/
├── rust/       # Core Rust SDK (bullet-rust-sdk)
├── wasm/       # WASM bindings for JS/TS (bullet-rust-sdk-wasm)
└── justfile    # Development commands
```

## Features

- **REST API Client**: Type-safe client generated from OpenAPI spec using Progenitor
- **WebSocket Support**: Real-time market data and order submission — including a portable `ManagedWebsocket` (auto-reconnect, exponential backoff, subscription replay, idle-stream detection, backoff reset on stable uptime, subscribe dedup) that works on both native and WASM
- **Transaction Building**: Fluent builder pattern for constructing and signing transactions
- **Cross-platform**: Works on native Rust and WASM (browser/Node.js)
- **Client Defaults**: Configure keypair, max fee, etc. once on the client

## Quick Start (Rust)

Add to your `Cargo.toml`:

```toml
[dependencies]
bullet-rust-sdk = { git = "https://github.com/bulletxyz/bullet-rust-sdk.git" }
tokio = { version = "1", features = ["full"] }
```

### Basic Usage

```rust
use bullet_rust_sdk::{Client, Keypair, Transaction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to mainnet with default keypair
    let keypair = Keypair::from_hex("your-private-key")?;
    let client = Client::builder()
        .url("https://tradingapi.bullet.xyz")
        .keypair(keypair)
        .max_fee(10_000_000.into())
        .build()
        .await?;

    // Query market data
    let info = client.exchange_info().await?.into_inner();
    println!("Symbols: {}", info.symbols.len());

    // Build and send a transaction (uses client defaults)
    let response = Transaction::builder()
        .call_message(call_msg)
        .send(&client)
        .await?;

    Ok(())
}
```

## Quick Start (JavaScript/TypeScript)

```bash
npm install bullet-rust-sdk-wasm
```

```typescript
import { Client, Keypair, Transaction, User } from 'bullet-rust-sdk-wasm';

// Connect with defaults
const keypair = Keypair.fromHex('your-private-key');
const client = await Client.builder()
    .url('https://tradingapi.bullet.xyz')
    .keypair(keypair)
    .maxFee(10_000_000n)
    .build();

// Build a transaction using generated factory methods
const callMsg = User.deposit({ asset_id: 0, amount: 1000000n });

// Send transaction (uses client defaults for signing)
const response = await Transaction.builder()
    .callMessage(callMsg)
    .send(client);
```

## Development

Use `just` for all common tasks:

```bash
just              # List available recipes
just check        # Check compilation
just test         # Run Rust unit tests
just test-wasm    # Run WASM Jest tests
just build-wasm   # Build WASM for web and Node.js
just lint         # Run clippy
just fmt          # Format code
```

### Integration Tests

```bash
# Against mainnet
just test-integration

# Against custom endpoint
just test-integration http://localhost:3000
```

## Architecture

### REST Client Generation

The REST client is generated at build time from the OpenAPI spec using [Progenitor](https://github.com/oxidecomputer/progenitor). The spec is fetched from mainnet during build, with a cached fallback in `rust/openapi.json`.

### WASM CallMessage Codegen

The WASM crate uses build-time codegen to generate `wasm_bindgen` factory methods for `CallMessage` variants. This walks the `bullet-exchange-interface` schema and generates:

- **Namespace structs** (`User`, `Public`, `Keeper`, `Vault`, `Admin`) with factory methods
- **Wrapper structs** for complex types (e.g., `WasmNewOrderArgs`)
- **Enum wrappers** for schema enums

See `wasm/codegen/` for the implementation.

## Contributing

See [AGENTS.md](./AGENTS.md) for development guidelines, including the critical requirement that **WASM bindings must mirror the Rust SDK** when adding or modifying public APIs.
