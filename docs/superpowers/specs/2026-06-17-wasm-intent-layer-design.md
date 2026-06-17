# WASM Intent Layer Design

## Goal

`@bulletxyz/sdk-wasm` should let web apps define transaction callbacks, websocket topics, enum values, and error handling during initial render/startup without importing the full `wasm-bindgen` runtime or `.wasm` binary.

The full WASM runtime should only enter the route graph when the app initializes a `Client`, builds/signs/submits a transaction, or uses runtime-backed response wrappers.

## Current Problem

The package root re-exports generated `wasm-bindgen` classes from `bullet_rust_sdk_wasm.js`. Imports such as `User`, `Warp`, `Topic`, transaction arg classes, enum wrappers, and `BulletSdkError` can therefore pull the full generated glue into the initial route graph even before `Client.builder().build()` is dynamically imported.

`User.*` and `Warp.*` currently return opaque WASM-backed `CallMessage` objects. In practice these objects carry runtime-call data internally:

- Exchange actions are `RuntimeCall::Exchange(CallMessage::User(...))`, etc.
- Warp actions are `RuntimeCall::Warp(...)`.

A split that only re-exports these classes from another file would not solve the startup bundle issue.

## Recommended Pattern: Intent Layer

Add a side-effect-free package subpath:

```ts
import { User, Warp, Topic, Side, OrderType, isBulletSdkError } from "@bulletxyz/sdk-wasm/intent";
```

The intent layer exports plain JavaScript values and builders. It does not import `bullet_rust_sdk_wasm.js`, does not call `init`, and does not reference the `.wasm` binary.

The full SDK remains the execution layer:

```ts
const call = User.cancelAllOrders();

const { Client, Transaction } = await import("@bulletxyz/sdk-wasm");
const client = await Client.builder().network(network).build();

const tx = Transaction.builder()
  .callIntent(call)
  .maxFee(10_000_000n)
  .build(client);
```

This gives the app an obvious split:

- Import intents during render/startup.
- Import the root SDK only when executing intents.

## Intent Values

Intent builders return canonical serde-shaped runtime-call data:

```ts
User.cancelAllOrders()
// { exchange: { user: { cancel_all_orders: {} } } }

Warp.transferRemote(args)
// { warp: { transfer_remote: { ... } } }
```

The intent value should be branded at the TypeScript level and remain a plain object at runtime:

```ts
export type RuntimeCallIntent = Readonly<JsonObject> & { readonly __brand?: "RuntimeCallIntent" };
```

The intent layer should provide helpers:

```ts
toRuntimeCallJson(intent): string
isRuntimeCallIntent(value): value is RuntimeCallIntent
```

The full WASM SDK should accept these values at the transaction boundary:

```ts
RuntimeCall.fromIntent(intent)
Transaction.builder().callIntent(intent)
client.sendCallIntent(intent)
```

These methods convert intent data to `RuntimeCall` inside WASM using serde, so final validation still happens in the runtime that signs/submits the transaction.

For backward compatibility, existing root imports and methods remain:

```ts
Transaction.builder().callMessage(User.cancelAllOrders())
RuntimeCall.exchange(User.cancelAllOrders())
```

## Structs And Enums

The intent layer should keep existing constructor ergonomics where it helps migration:

```ts
const order = new NewOrderArgs("50000.0", "0.1", Side.Bid, OrderType.Limit, false);
User.placeOrders(0, [order], false);
```

Intent-layer structs are plain data wrappers with `toJSON()`, not WASM classes.

Intent-layer enum constants should use the canonical serde string values, not the numeric `wasm-bindgen` enum values:

```ts
Side.Bid === "bid";
OrderType.Limit === "limit";
```

The root export keeps the existing numeric WASM enum wrappers for compatibility.

## Topics

The intent layer should export pure websocket topic builders:

```ts
Topic.depth("BTC-USD", OrderbookDepth.D10).toString()
// "BTC-USD@depth10"
```

The root WebSocket subscribe APIs should accept both legacy WASM `Topic` objects and intent topics by converting any topic argument with a `toString(): string` method to the existing wire-format string.

## Errors

Export startup-safe error shape helpers from the intent subpath:

```ts
BulletSdkError
isBulletSdkError(error)
asBulletSdkError(error)
```

`isBulletSdkError` should check shape rather than relying on `instanceof`, so app startup code does not need to import the full root SDK just to classify errors.

The package root continues exporting `BulletSdkError` for backward compatibility.

## Package Exports

Add subpath exports:

```json
{
  "exports": {
    ".": { "...": "existing root export" },
    "./intent": {
      "types": "./pkg/intent.d.ts",
      "default": "./pkg/intent.js"
    }
  },
  "sideEffects": [
    "./pkg/node.js"
  ]
}
```

`./pkg/intent.js` must stay side-effect-free and should be covered by tests that prove importing it does not import the generated WASM glue.

## Code Generation

The existing `wasm/codegen/bullet_schema` data model already knows:

- action namespaces and variants
- struct fields and optionality
- enum variants
- parameter ordering

Extend code generation to emit an intent metadata artifact or directly emit `intent.js` and `intent.d.ts` during `just build-wasm`.

The generated intent code should use canonical serde names:

- namespace names: `User` -> `exchange.user`
- action names: `CancelAllOrders` -> `cancel_all_orders`
- field names: `market_id`, `sub_account_index`, etc.
- enum values: `Bid` -> `bid`

## Schema Prefetch

Schema prefetch should be a follow-up layer, not the core of the intent split.

The intent layer can later expose:

```ts
prefetchClientBootstrap(network)
```

The full client can later accept:

```ts
Client.builder().bootstrap(prefetched).build()
```

This should cover schema and exchange metadata together if the goal is to reduce client-build latency. Bundling a schema by default is not recommended because the chain hash is runtime network state and stale signing context is risky.

## Tests

Add focused tests for:

- importing `@bulletxyz/sdk-wasm/intent` does not import `bullet_rust_sdk_wasm.js`
- intent builders produce the same `toMessageBytes().runtime_call` JSON as legacy WASM builders
- `Transaction.builder().callIntent(...)` matches `callMessage(...)` bytes for representative actions
- `Warp.transferRemote` preserves current amount and relayer behavior
- intent topic builders match existing topic wire strings
- `isBulletSdkError` narrows by shape without requiring root SDK import
- package exports expose `./intent` for Node and browser/default conditions

## Non-Goals

- Do not remove or rename existing root exports.
- Do not make the root package side-effect-free in this change.
- Do not replace transaction signing or validation with JavaScript logic.
- Do not bundle schema data as the default client-build path.
