# WASM codegen

`rust/build.rs` generates the REST client from committed `rust/openapi.json`
with Progenitor and exposes its output through Cargo `links` metadata.
`wasm/build.rs` reads that output, parses it with `syn`, and generates the REST
types and client methods under `OUT_DIR`. Do not edit generated wrappers.

Call-message factories come from the `bullet-exchange-interface` schema through
`wasm/codegen/bullet_schema/`; REST wrappers come from `wasm/codegen/progenitor/`.
Handwritten JS interop lives in `wasm/src/` and must mirror the Rust public API.

After changing the REST schema, rebuild both crates. After changing either public
API, update the [npm usage guide](README.md) and keep its JSDoc current.
