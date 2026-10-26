# Bullet Rust SDK

[![Crates.io](https://img.shields.io/crates/v/bullet-rust-sdk.svg)](https://crates.io/crates/bullet-rust-sdk)
[![Documentation](https://docs.rs/bullet-rust-sdk/badge.svg)](https://docs.rs/bullet-rust-sdk)
[![License: MIT](https://img.shields.io/crates/l/bullet-rust-sdk.svg)](LICENSE)

Rust client for the Bullet trading API, with [JavaScript/TypeScript bindings](wasm/README.md).

## Rust quickstart

```toml
[dependencies]
bullet-rust-sdk = "*"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use bullet_rust_sdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::mainnet().await?;
    let info = client.exchange_info().await?.into_inner();
    println!("Symbols: {}", info.symbols.len());
    Ok(())
}
```

For signing, order submission and streaming, see the [Rust API](https://docs.rs/bullet-rust-sdk)
and [runnable examples](rust/examples/).

## JavaScript/TypeScript

```sh
npm install @bulletxyz/sdk-wasm
```

See the [npm usage guide](wasm/README.md) and [platform examples](examples/README.md).

## Development

The REST client is generated from committed `rust/openapi.json`; normal builds do not
fetch a schema. Refresh it with `just refresh-spec` when the Trading API changes.
Rust and WASM public APIs must stay in sync. See [WASM codegen](wasm/ARCHITECTURE.md)
for the generated-source boundary and `just --list` for development commands.
