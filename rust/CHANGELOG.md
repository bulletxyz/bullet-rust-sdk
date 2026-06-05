# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.24](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.23...v0.0.24) - 2026-06-05

### Features

- vault creation script + deriveVaultAddress helper ([#90](https://github.com/bulletxyz/bullet-rust-sdk/pull/90))

## [0.0.23](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.22...v0.0.23) - 2026-06-05

### Features

- RuntimeCall builder seam + simulate support + typed Symbol filters ([#88](https://github.com/bulletxyz/bullet-rust-sdk/pull/88))
- ledger multisig signing + Nonce/Window uniqueness ([#87](https://github.com/bulletxyz/bullet-rust-sdk/pull/87))

## [0.0.22](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.21...v0.0.22) - 2026-06-01

### Bug Fixes

- *(sdk)* auto-refresh chain hash and retry on 401 invalid signature ([#60](https://github.com/bulletxyz/bullet-rust-sdk/pull/60))

## [0.0.21](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.20...v0.0.21) - 2026-05-24

### Bug Fixes

- track trading-api ApiErrorResponse schema + tighten error handling ([#78](https://github.com/bulletxyz/bullet-rust-sdk/pull/78))

### Features

- bump bullet-ws-interface to 0.2 + adopt upstream response types ([#81](https://github.com/bulletxyz/bullet-rust-sdk/pull/81))

## [0.0.20](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.19...v0.0.20) - 2026-05-19

### Bug Fixes

- refresh cached OpenAPI spec ([#76](https://github.com/bulletxyz/bullet-rust-sdk/pull/76))

## [0.0.19](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.18...v0.0.19) - 2026-05-14

### Features

- *(transaction)* default generation to unix millis, allow override ([#71](https://github.com/bulletxyz/bullet-rust-sdk/pull/71))

## [0.0.18](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.17...v0.0.18) - 2026-05-14

### Features

- *(ws)* add OrderResult variants to TaggedMessage ([#66](https://github.com/bulletxyz/bullet-rust-sdk/pull/66))

## [0.0.17](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.16...v0.0.17) - 2026-05-14

### Features

- add Ledger spec-compliant offchain signing path ([#68](https://github.com/bulletxyz/bullet-rust-sdk/pull/68))

## [0.0.16](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.15...v0.0.16) - 2026-05-14

### Bug Fixes

- *(transaction)* use microseconds for UniquenessData::Generation, not milliseconds ([#65](https://github.com/bulletxyz/bullet-rust-sdk/pull/65))

## [0.0.15](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.14...v0.0.15) - 2026-05-13

### Bug Fixes

- *(trading)* wrap symbol in Some for query_open_orders after generated-client drift ([#61](https://github.com/bulletxyz/bullet-rust-sdk/pull/61))
- serialize Solana offchain auth envelope ([#56](https://github.com/bulletxyz/bullet-rust-sdk/pull/56))

### Features

- *(ws)* add order_amend and order_cancel_all to ManagedWebsocket + WebsocketHandle ([#59](https://github.com/bulletxyz/bullet-rust-sdk/pull/59))

### Revert

- undo abandoned v0.0.15 release bump ([#57](https://github.com/bulletxyz/bullet-rust-sdk/pull/57)) ([#62](https://github.com/bulletxyz/bullet-rust-sdk/pull/62))

## [0.0.14](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.13...v0.0.14) - 2026-05-11

### Features

- *(ci)* Add Release Pipeline (NPM) ([#13](https://github.com/bulletxyz/bullet-rust-sdk/pull/13))

## [0.0.6](https://github.com/bulletxyz/bullet-rust-sdk/compare/v0.0.5...v0.0.6) - 2026-05-11

### Bug Fixes

- WASM schema codegen and websocket config ([#34](https://github.com/bulletxyz/bullet-rust-sdk/pull/34))

### Features

- add user order topic ([#33](https://github.com/bulletxyz/bullet-rust-sdk/pull/33))

## [0.0.4](https://github.com/bulletxyz/bullet-rust-sdk/releases/tag/v0.0.4) - 2026-05-04

### Bug Fixes

- use dedicated HTTP/1.1 client for WebSocket connections ([#5](https://github.com/bulletxyz/bullet-rust-sdk/pull/5))

### Features

- SDK developer experience overhaul ([#12](https://github.com/bulletxyz/bullet-rust-sdk/pull/12))
- ensure call-message was validated in the schema ([#10](https://github.com/bulletxyz/bullet-rust-sdk/pull/10))
- Return JSDoc for types that cant have concrete types, make WS types more concrete. ([#11](https://github.com/bulletxyz/bullet-rust-sdk/pull/11))

### Rust_decimal

- :Decimal support ([#3](https://github.com/bulletxyz/bullet-rust-sdk/pull/3))
