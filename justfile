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

# Build the WASM package (web target + Node.js wrapper)
build-wasm:
    wasm-pack build wasm --target web --out-dir pkg
    # Remove wasm-pack generated package.json and .gitignore that interfere with npm install --install-links
    rm -f wasm/pkg/.gitignore wasm/pkg/package.json
    # Copy the real README into pkg/ (wasm-pack generates a stub from Cargo.toml description)
    cp wasm/README.md wasm/pkg/README.md
    # Generate Node.js auto-init wrapper (uses web target's initSync)
    printf '%s\n' \
        'import { readFileSync } from "node:fs";' \
        'import { initSync } from "./bullet_rust_sdk_wasm.js";' \
        'const wasm = readFileSync(new URL("./bullet_rust_sdk_wasm_bg.wasm", import.meta.url));' \
        'initSync({ module: wasm });' \
        'export * from "./bullet_rust_sdk_wasm.js";' \
        > wasm/pkg/node.js
    # Generate type re-exports
    echo 'export * from "./bullet_rust_sdk_wasm.js";' > wasm/pkg/node.d.ts

# Remove generated WASM build artifacts
clean-wasm:
    rm -rf wasm/pkg

# ── Test ──────────────────────────────────────────────────────────────────────

# Run unit tests
test:
    cargo nextest run

# Run doc tests
test-doc:
    cargo test --doc

# Run integration tests (requires a running API)
test-integration endpoint="https://tradingapi.bullet.xyz":
    BULLET_API_ENDPOINT={{ endpoint }} cargo nextest run --features integration

# Run WASM Jest tests (requires build-wasm-node first)
test-wasm:
    cd wasm && npm test

# Run all tests (Rust unit + doc + WASM)
test-all: test test-doc test-wasm

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

# Run the Node.js WASM example
example-node: build-wasm
    cd examples/node && npm install && npm start

# Run the Deno WASM example
example-deno: build-wasm
    cd examples/deno && deno task start

# Run the web WASM example (Next.js)
example-web: build-wasm
    cd examples/web && npm install --install-links && npm run dev

# Run Node.js WASM example tests
test-example-node: build-wasm
    cd examples/node && npm install && npm test

# Run Deno WASM example tests
test-example-deno: build-wasm
    cd examples/deno && deno task test

# Run all example tests (Node + Deno)
test-examples: test-example-node test-example-deno

# ── CI ────────────────────────────────────────────────────────────────────────

# Full end-to-end build + test (Rust → WASM → examples)
ci:
    #!/usr/bin/env bash
    set -euo pipefail

    step() { printf '\n\033[1;34m══ %s\033[0m\n' "$1"; }

    step "Rust: format check"
    cargo fmt -- --check

    step "Rust: clippy"
    cargo clippy --all-targets -- -D warnings

    step "Rust: build"
    cargo build

    step "Rust: unit tests"
    cargo nextest run

    step "Rust: doc tests"
    cargo test --doc

    step "WASM: build"
    just build-wasm

    step "WASM: Jest tests"
    cd wasm && npm test && cd ..

    step "Examples: install"
    cd examples && npm install && cd ..

    step "Examples: Node.js tests"
    cd examples/node && npm test && cd ../..

    step "Examples: Deno tests"
    cd examples/deno && deno task test && cd ../..

    step "Examples: Next.js build"
    cd examples/web && npm install --install-links && npx next build && cd ../..

    printf '\n\033[1;32m✓ All checks passed\033[0m\n'

# ── OpenAPI spec ──────────────────────────────────────────────────────────────

# Fetch and cache the latest OpenAPI spec from mainnet
fetch-spec endpoint="https://tradingapi.bullet.xyz":
    curl -sSf {{ endpoint }}/docs/rest/openapi.json | nix run nixpkgs#jq -- . > rust/openapi.json
