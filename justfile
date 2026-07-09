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
    rm -rf wasm/pkg
    wasm-pack build wasm --target web --out-dir pkg
    # Remove wasm-pack generated package.json and .gitignore that interfere with npm install --install-links
    rm -f wasm/pkg/.gitignore wasm/pkg/package.json
    # Copy JS error class used by Rust wasm-bindgen imports and package exports
    cp wasm/js/bullet-sdk-error.js wasm/pkg/bullet-sdk-error.js
    cp wasm/js/bullet-sdk-error.d.ts wasm/pkg/bullet-sdk-error.d.ts
    cp wasm/.generated/startup-shared.js wasm/pkg/startup-shared.js
    cp wasm/.generated/startup-shared.d.ts wasm/pkg/startup-shared.d.ts
    cp wasm/.generated/calls.js wasm/pkg/calls.js
    cp wasm/.generated/calls.d.ts wasm/pkg/calls.d.ts
    cp wasm/.generated/topics.js wasm/pkg/topics.js
    cp wasm/.generated/topics.d.ts wasm/pkg/topics.d.ts
    cp wasm/.generated/errors.js wasm/pkg/errors.js
    cp wasm/.generated/errors.d.ts wasm/pkg/errors.d.ts
    cp wasm/.generated/primitives.js wasm/pkg/primitives.js
    cp wasm/.generated/primitives.d.ts wasm/pkg/primitives.d.ts
    # Copy the real README into pkg/ (wasm-pack generates a stub from Cargo.toml description)
    cp wasm/README.md wasm/pkg/README.md
    # Generate Node.js auto-init wrapper (uses web target's initSync)
    printf '%s\n' \
        'import { readFileSync } from "node:fs";' \
        'import { initSync } from "./bullet_rust_sdk_wasm.js";' \
        'const wasm = readFileSync(new URL("./bullet_rust_sdk_wasm_bg.wasm", import.meta.url));' \
        'initSync({ module: wasm });' \
        'export { BulletSdkError } from "./bullet-sdk-error.js";' \
        'export * from "./bullet_rust_sdk_wasm.js";' \
        > wasm/pkg/node.js
    # Generate type re-exports
    printf '%s\n' \
        'export { BulletSdkError } from "./bullet-sdk-error.js";' \
        'export type { BulletSdkErrorDetails, BulletSdkErrorDetailsByKind, BulletSdkErrorKind, BulletSdkErrorOptions, BulletSdkErrorStatus, JsonValue } from "./bullet-sdk-error.js";' \
        'export * from "./bullet_rust_sdk_wasm.js";' \
        > wasm/pkg/node.d.ts
    # Generate browser/default package wrapper
    printf '%s\n' \
        'export { BulletSdkError } from "./bullet-sdk-error.js";' \
        'export * from "./bullet_rust_sdk_wasm.js";' \
        'export { default } from "./bullet_rust_sdk_wasm.js";' \
        > wasm/pkg/index.js
    printf '%s\n' \
        'export { BulletSdkError } from "./bullet-sdk-error.js";' \
        'export type { BulletSdkErrorDetails, BulletSdkErrorDetailsByKind, BulletSdkErrorKind, BulletSdkErrorOptions, BulletSdkErrorStatus, JsonValue } from "./bullet-sdk-error.js";' \
        'export * from "./bullet_rust_sdk_wasm.js";' \
        'export { default } from "./bullet_rust_sdk_wasm.js";' \
        > wasm/pkg/index.d.ts

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

# Run WASM Jest tests (requires build-wasm first)
test-wasm:
    if [ ! -x wasm/node_modules/.bin/jest ]; then cd wasm && corepack pnpm install --frozen-lockfile; fi
    cd wasm && npm test

# Run all tests (Rust unit + doc + WASM)
test-all: test test-doc test-wasm

# ── Lint ──────────────────────────────────────────────────────────────────────

# Run clippy lints
lint:
    cargo clippy --all-targets -- -D warnings

# Format all source files
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

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
    cd examples/deno && (command -v deno >/dev/null 2>&1 && deno task start || npx --yes deno task start)

# Run the web WASM example (Next.js)
example-web: build-wasm
    cd examples/web && npm install --install-links && npm run dev

# Run Node.js WASM example tests
test-example-node: build-wasm
    cd examples/node && npm install && npm test

# Run Deno WASM example tests
test-example-deno: build-wasm
    cd examples/deno && (command -v deno >/dev/null 2>&1 && deno task test || npx --yes deno task test)

# Run all example tests (Node + Deno)
test-examples: test-example-node test-example-deno

# ── CI ────────────────────────────────────────────────────────────────────────

# Full end-to-end build + test (Rust → WASM → examples)
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    export npm_config_cache="${TMPDIR:-/tmp}/bullet-rust-sdk-npm-cache"

    step() { printf '\n\033[1;34m══ %s\033[0m\n' "$1"; }
    run_deno() {
        if command -v deno >/dev/null 2>&1; then
            deno "$@"
        else
            npx --yes deno "$@"
        fi
    }

    step "Rust: format check"
    just fmt-check

    step "Rust: clippy"
    just lint

    step "Rust: unit tests"
    just test

    step "Rust: doc tests"
    just test-doc

    step "WASM: build"
    just build-wasm

    step "WASM: Jest tests"
    just test-wasm

    step "Examples: install"
    (cd examples && npm ci --install-links)

    step "Examples: Node.js tests"
    (cd examples/node && npm test)

    step "Examples: Deno tests"
    (cd examples/deno && run_deno task test)

    step "Examples: Next.js build"
    (cd examples/web && npx next build)

    printf '\n\033[1;32m✓ All checks passed\033[0m\n'

# ── OpenAPI spec ──────────────────────────────────────────────────────────────

# Refresh rust/openapi.json from the live trading-api endpoint. Use this
# when upstream spec changes need to be tracked in the SDK; commit the
# resulting diff alongside any hand-written code updates the new spec
# requires. The SDK build itself uses the committed file — see build.rs.
#
# `--indent 4` matches the trading-api's (utoipa) native 4-space formatting, so
# a refresh produces a content-only diff instead of reformatting the whole file.
refresh-spec endpoint="https://tradingapi.bullet.xyz":
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT
    curl -sSf "{{ endpoint }}/docs/rest/openapi.json" -o "$tmp"
    jq --indent 4 -- . "$tmp" > rust/openapi.json

# Verify rust/openapi.json is canonically formatted (jq --indent 4 idempotent).
# Guards against hand-edits or a different formatter producing whole-file diffs.
check-spec:
    @jq --indent 4 -- . rust/openapi.json | diff -u rust/openapi.json - \
        || { echo "ERROR: rust/openapi.json is not canonical. Run: just refresh-spec"; exit 1; }
