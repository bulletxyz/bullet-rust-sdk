# bullet-rust-sdk justfile

# Default: list available recipes
default:
    @just --list

# ── Build ─────────────────────────────────────────────────────────────────────

# Check the workspace compiles without errors
check:
    cargo check

# Build the workspace in debug mode
build:
    cargo build

# Build the workspace in release mode
build-release:
    cargo build --release

# Build the WASM package for browser environments
build-wasm-web:
    wasm-pack build wasm --target web --out-dir pkg/web

# Build the WASM package for Node.js environments
build-wasm-node:
    wasm-pack build wasm --target nodejs --out-dir pkg/node

# Build the WASM package for both web and Node.js
build-wasm: build-wasm-web build-wasm-node

# Remove generated WASM build artifacts
clean-wasm:
    rm -rf wasm/pkg/web wasm/pkg/node

# ── Test ──────────────────────────────────────────────────────────────────────

# Run unit tests
test:
    cargo nextest run

# Run integration tests (requires a running API)
test-integration endpoint="https://tradingapi.bullet.xyz":
    BULLET_API_ENDPOINT={{ endpoint }} cargo nextest run --features integration

# Run WASM Jest tests (requires build-wasm-node first)
test-wasm:
    cd wasm && npm test

# Run all tests (Rust + WASM)
test-all: test test-wasm

# ── Lint ──────────────────────────────────────────────────────────────────────

# Run clippy lints
lint:
    cargo clippy --all-targets -- -D warnings

# Format all source files
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# ── Examples ──────────────────────────────────────────────────────────────────

# Run the REST example (set API_ENDPOINT env var to override)
example-rest:
    cargo run -p bullet-rust-sdk --example rest

# Run the WebSocket example (set API_ENDPOINT env var to override)
example-ws:
    cargo run -p bullet-rust-sdk --example websocket

# ── OpenAPI spec ──────────────────────────────────────────────────────────────

# Fetch and cache the latest OpenAPI spec from mainnet
fetch-spec endpoint="https://tradingapi.bullet.xyz":
    curl -sSf {{ endpoint }}/docs/rest/openapi.json | nix run nixpkgs#jq -- . > rust/openapi.json
