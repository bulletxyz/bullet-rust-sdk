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

# Build the WASM package with wasm-pack
build-wasm:
    nix run nixpkgs#wasm-pack -- build wasm --target web

# ── Test ──────────────────────────────────────────────────────────────────────

# Run unit tests
test:
    cargo nextest run

# Run integration tests (requires a running API)
test-integration endpoint="https://tradingapi.bullet.xyz":
    BULLET_API_ENDPOINT={{ endpoint }} cargo nextest run --features integration

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
    cargo run --example rest

# Run the WebSocket example (set API_ENDPOINT env var to override)
example-ws:
    cargo run --example websocket

# ── OpenAPI spec ──────────────────────────────────────────────────────────────

# Fetch and cache the latest OpenAPI spec from mainnet
fetch-spec endpoint="https://tradingapi.bullet.xyz":
    curl -sSf {{ endpoint }}/docs/rest/openapi.json | nix run nixpkgs#jq -- . > openapi.json
