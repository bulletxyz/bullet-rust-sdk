# Trading SDK

A Rust SDK for interacting with the Bullet trading API. The client is automatically generated from the trading-api's OpenAPI specification using Progenitor.

## Design

- Uses Progenitor to generate a type-safe client around the OpenAPI spec
- All API types and methods are generated at build time

## Features

- **Cross-platform WebSocket**: Works on both native and WASM targets
- **Auto-updating**: Rebuilding fetches the latest OpenAPI spec from running server
- **Offline builds**: Falls back to cached spec when server isn't available
- **Type-safe**: All endpoints and types generated from OpenAPI spec

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
bullet-rust-sdk = { git="https://github.com/bulletxyz/bullet-rust-sdk.git" }
tokio = { version = "1", features = ["full"] }
```

Example:

```rust
use bullet_rust_sdk::TradingApi;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to testnet (or use TradingApi::mainnet(), TradingApi::staging())
    let api = TradingApi::testnet().await?;

    // Get exchange info
    let exchange_info = api.exchange_info().await?.into_inner();
    println!("Symbols: {}", exchange_info.symbols.len());

    // Get ticker prices
    let tickers = api.ticker_price(None).await?.into_inner();
    for ticker in tickers {
        println!("{}: {}", ticker.symbol, ticker.price);
    }

    Ok(())
}
```

## Testing

Run local tests only.

```bash
cargo nextest run
```

Run integration tests to check against the running API .

```bash
cargo nextest run --features integration
```

Connect to a custom API endpoint.

```bash
BULLET_API_ENDPOINT=https://custom.api.example.com cargo nextest run --features integration
```

## Building

The build process fetches the current spec from Mainnet. Use
BULLET_API_ENDPOINT to override the URL. If fetching fails or cargo was
run in offline-mode it will use an older version from `openapi.json`.
