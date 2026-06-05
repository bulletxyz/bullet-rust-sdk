# @bulletxyz/sdk-wasm

WebAssembly bindings for the [Bullet](https://bullet.xyz) trading API. Works in Node.js, browsers, and Deno.

## Install

```bash
npm install @bulletxyz/sdk-wasm
```

## Quick Start

```typescript
import { Client, Keypair, Transaction, User, NewOrderArgs, Side, OrderType } from '@bulletxyz/sdk-wasm';

// Connect to mainnet
const client = await Client.mainnet();

// Or with defaults for signing
const keypair = Keypair.fromHex('your-private-key');
const client = await Client.builder()
    .network('mainnet')
    .keypair(keypair)
    .build();

// Query market data
const info = await client.exchangeInfo();
const ticker = await client.tickerPrice('BTC-USD');

// Place an order
const order = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false);
const response = await Transaction.builder()
    .callMessage(User.placeOrders(0, [order], false))
    .signer(keypair)
    .send(client);
```

## API Reference

### Client

```typescript
// Constructors
const client = await Client.mainnet();
const client = await Client.connect('https://tradingapi.bullet.xyz');
const client = await Client.builder()
    .network('mainnet')        // or 'testnet', or custom URL
    .keypair(keypair)          // default signer
    .maxPriorityFeeBips(100n)  // default priority fee
    .userActions(['PlaceOrders', 'CancelOrders'])  // optional User-only schema pruning
    .build();

// Metadata
client.chainId()             // u64
client.chainHash()           // Uint8Array (32 bytes)
client.chainName()           // chain name used in Solana offchain messages
client.url()                 // REST URL
client.wsUrl()               // WebSocket URL
client.maxFee()              // default max fee
client.hasKeypair()          // whether a default keypair is set

// Submission
await client.sendTransaction(signedTx)  // returns JSON string
await client.sendOffChainTransaction(offchainTx)
```
By default the client validates every exchange `CallMessage` group (`User`,
`Vault`, `Keeper`, `Public`, and `Admin`) against the server schema when it
connects. `.userActions(...)` intentionally narrows validation to only the
listed `UserAction` variants; when enabled, non-`User` call messages and
unlisted user actions are rejected before signing.

### Errors

Fallible WASM calls throw `BulletSdkError`, a JavaScript `Error` subclass with
parseable SDK metadata.

```typescript
import { BulletSdkError } from '@bulletxyz/sdk-wasm';

try {
    await client.accountBalance(address);
} catch (err) {
    if (err instanceof BulletSdkError) {
        err.kind       // 'api' | 'http' | 'websocket' | 'validation' | ...
        err.status     // HTTP status when the API returned one
        err.details    // structured JSON details when available
        err.errorId    // server-side correlation id, when present (for support)
        err.retryable  // whether retry/backoff is reasonable
        err.message    // human-readable message
    }
}
```

### Transaction Builder

All transaction construction goes through the builder:

```typescript
// Build and send in one step
const response = await Transaction.builder()
    .callMessage(User.deposit(0, '1000.0'))
    .signer(keypair)
    .send(client);

// Build without sending
const tx = Transaction.builder()
    .callMessage(msg)
    .signer(keypair)
    .build(client);

await client.sendTransaction(tx);
```

### Uniqueness (replay protection)

Every transaction carries a uniqueness value. By default the SDK uses
**window-based** uniqueness from a per-client counter that tracks the
millisecond unix timestamp and increments per transaction — a monotonic,
duplicate-free value that needs no chain round-trip and tolerates many
in-flight transactions.

Override it with any one of `.window()`, `.generation()`, or `.nonce()` (these
set the same single uniqueness value, so the last call wins):

```typescript
const tx = Transaction.builder()
    .callMessage(msg)
    .nonce(42n)        // or .generation(n) / .window(n)
    .signer(keypair)
    .send(client);
```

### External Signing

For hardware wallets or external signing services that can sign the standard
Borsh payload:

```typescript
// Build unsigned (chain hash is baked in)
const unsigned = Transaction.builder()
    .callMessage(User.deposit(0, '1000.0'))
    .buildUnsigned(client);

// Get signable bytes and sign externally
const signable = unsigned.toBytes();
const display = unsigned.toDisplayMessage(); // optional: show in your own confirmation UI
const signature = myExternalSigner(signable);  // 64-byte Ed25519 signature

// Assemble the signed transaction
const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
await client.sendTransaction(signed);
```

### UnsignedTransaction

Returned by `Transaction.builder().buildUnsigned(client)`. Contains the
chain hash so signable bytes can be produced without a client reference.

```typescript
unsigned.toBytes()  // Uint8Array — borsh-serialized tx + chain hash (signable bytes)
unsigned.toDisplayMessage() // string — human-readable unsigned payload for display only
unsigned.toMessageBytes() // Uint8Array — readable JSON bytes for Solana wallets
```

Some external wallets display `signMessage` bytes as raw UTF-8, so `toBytes()`
can look garbled in the wallet confirmation. That is expected: those bytes are
what the network verifies. Use `toDisplayMessage()` in your app UI to show the
transaction contents before asking the wallet to sign `toBytes()`.

For external Solana wallets where the wallet confirmation should show readable
JSON, use the Solana offchain path instead. `toMessageBytes()` returns the
readable JSON payload with `chain_name` and `chain_id` as the domain fields.
`SolanaOffchainTransaction.fromParts(...)` also carries the chain hash required
by the current sequencer offchain authenticator envelope:

```typescript
const unsigned = Transaction.builder()
    .callMessage(User.deposit(0, '1000.0'))
    .buildUnsigned(client);

const message = unsigned.toMessageBytes();
const signature = await wallet.signMessage(message);
const pubKey = wallet.publicKey.toBytes(); // 32-byte Solana public key
const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);

await client.sendOffChainTransaction(tx);
```

### SignedTransaction

Returned by `Transaction.builder().build(client)` or assembled from parts.

```typescript
// From builder
const tx = Transaction.builder()
    .callMessage(msg).maxFee(fee).signer(kp).build(client);

// From external signing
const tx = SignedTransaction.fromParts(unsigned, signature, pubKey);

// Serialization
tx.toBytes()   // Uint8Array (borsh)
tx.toBase64()  // base64 string (for WebSocket submission)
```

### SolanaOffchainTransaction

Assembled after signing `unsigned.toMessageBytes()` with a Solana
wallet. Submit it with `client.sendOffChainTransaction(tx)`, which posts to
the trading API's `/api/v1/solanaOffchainTx`.

```typescript
const pubKey = wallet.publicKey.toBytes(); // 32-byte Solana public key
const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);

tx.toBytes()   // Uint8Array (borsh offchain envelope)
tx.toBase64()  // base64 string
```

### Multisig

M-of-N multisig over the spec-compliant Solana offchain format (the format
Ledger hardware wallets sign). Every signer signs the same bytes; once the
threshold is met the transaction can be submitted.

```typescript
// 2-of-3 multisig. Keys are canonicalized (sorted) internally, so input
// order never matters.
const config = new MultisigConfig(2, [pubkeyA, pubkeyB, pubkeyC]);

config.credentialId()  // Uint8Array — sha256(minSigners || borsh(sorted pubkeys))
config.multisigId()    // string — base58 credential id (committed into the signed message)
config.minSigners()    // number
config.pubkeys()       // Uint8Array[] — canonical order

const unsigned = Transaction.builder()
    .callMessage(User.deposit(0, '1000.0'))
    .buildUnsigned(client);

// Collect signatures — each signer signs the same signable bytes
const tx = new SolanaLedgerMultisigTransaction(unsigned, config);
const signature = await ledgerWallet.signMessage(tx.signableBytes());
tx.addSignature(pubkeyA, signature);   // validates membership + signature
// ... pass tx.signableBytes() to the other signers ...
tx.addSignature(pubkeyB, signatureB);

tx.signatureCount() // number
tx.isComplete()     // true once minSigners signatures are collected

// Submit (throws if below threshold)
await client.sendLedgerMultisigTransaction(tx);
```

The signable bytes can also be produced without assembling a transaction:
`unsigned.toLedgerMultisigSignableBytes(config)` (preamble + JSON) and
`unsigned.toMultisigMessageBytes(config)` (JSON payload only).

### Keypair

Ed25519 keypair for signing transactions:

```typescript
// Create
const kp = Keypair.generate();
const kp = Keypair.fromHex('0xdeadbeef...');
const kp = Keypair.fromBytes(new Uint8Array(32));

// Export
kp.toHex()        // secret key as hex string
kp.toBytes()       // secret key as Uint8Array (32 bytes)
kp.publicKey()     // public key as Uint8Array (32 bytes)
kp.publicKeyHex()  // public key as hex string

// Sign arbitrary bytes
kp.sign(message)   // returns 64-byte Ed25519 signature
```

### CallMessage Factories

Transaction actions are constructed via namespace modules:

```typescript
// User actions
User.deposit(assetId, amount)
User.withdraw(assetId, amount)
User.placeOrders(marketId, [order1, order2], replace)
User.cancelOrders(marketId, [cancel1, cancel2])
User.cancelAllOrders()

// Public actions (no signing required, but still need a tx)
Public.applyFunding(addresses)

// Order types
const order = new NewOrderArgs(price, size, Side.Bid, OrderType.Limit, reduceOnly);
const cancel = new CancelOrderArgs(orderId, clientOrderId);
const amend = new AmendOrderArgs(cancel, newOrder);
```

### Vaults

Create a vault, then derive its address locally to reference it (the trading API
exposes no vault-address lookup, and `CreateVault` emits no address event). The
address is `deriveVaultAddress(name)` — deterministic from the vault name.

```typescript
import { CreateVaultArgs, UpdateVaultConfigArgs, User, Vault, deriveVaultAddress } from '@bulletxyz/sdk-wasm';

const args = new CreateVaultArgs(
  'My Vault',            // name (also determines the vault address)
  'Description',
  leaderAddress,         // base58 leader address
  Uint16Array.from([usdcId]), // deposit asset ids
  usdcId,                // withdraw asset id
  0,                     // withdraw lockup period (hours)
  false,                 // whitelist deposits
  100,                   // profit share % (0–100)
  0,                     // withdrawal fee bps
  '1000000',             // deposit limit
);
await client.sendCallMessage(User.createVault(args));

const vault = deriveVaultAddress('My Vault');

// Leader management ops
await client.sendCallMessage(Vault.whitelistDepositor(vault, depositor)); // when whitelistDeposits: true
await client.sendCallMessage(Vault.delegateVaultUserV1(vault, delegate, 'bot', 0));
// updateVaultConfig can change only these three; pass null to leave unchanged.
// withdrawalFeeBps and the whitelistDeposits toggle are fixed at creation.
await client.sendCallMessage(
  Vault.updateVaultConfig(vault, new UpdateVaultConfigArgs('2000000', null, 80)),
);
```

See [`examples/node/create_vault.ts`](../examples/node/create_vault.ts) for a
runnable end-to-end script.

### WebSocket

```typescript
const ws = await client.connectWs();

// Subscribe to topics
await ws.subscribe([
    Topic.aggTrade('BTC-USD'),
    Topic.depth('ETH-USD', OrderbookDepth.D10),
    Topic.bookTicker('SOL-USD'),
    Topic.kline('BTC-USD', KlineInterval.H1),
    Topic.userOrders('0xabc'),
    Topic.allTickers(),
]);

// Receive messages
const msg = await ws.recv();

// Submit orders via WebSocket
const tx = Transaction.builder()
    .callMessage(User.placeOrders(0, [order], false))
    .signer(keypair)
    .build(client);

await ws.orderPlace(tx.toBase64());
// also: ws.orderCancel(tx.toBase64()), ws.orderAmend(tx.toBase64()), ws.orderCancelAll(tx.toBase64())
// or the convenience forms that take a Transaction directly:
// ws.placeOrder(tx), ws.cancelOrder(tx), ws.amendOrder(tx), ws.cancelAllOrders(tx)
```

### Managed WebSocket (auto-reconnect)

For long-running bots, prefer `connectWsManaged` — it handles reconnection with
exponential backoff and replays your subscriptions automatically. It mirrors
the Rust `ManagedWebsocket` 1:1.

```typescript
const ws = await client.connectWsManaged(
    // optional — all fields optional
    new ManagedWsConfig(
        /* initialBackoffMs */ 500,
        /* maxBackoffMs */ 30_000,
        /* maxRetries */ undefined,          // infinite
        /* channelCapacity */ 10_000,
        /* idleTimeoutMs */ 60_000,          // force reconnect on zombie connections
        /* backoffResetAfterMs */ 30_000,    // reset backoff once connection is stable
    ),
);

ws.subscribe([Topic.depth('BTC-USD', OrderbookDepth.D20)]);

while (true) {
    const evt = await ws.recv();
    if (!evt) break;
    switch (evt.type) {
        case 'message':       handleMessage(evt.message); break;
        case 'reconnecting':  console.log('reconnecting...'); break;
        case 'disconnected':  console.error(evt.reason); return;
    }
}

// Order submission — fire-and-forget; acks arrive as 'message' events
ws.placeOrder(tx);
ws.cancelOrder(cancelTx);
ws.amendOrder(amendTx);
ws.cancelAllOrders(cancelAllTx);
```

### Decimal

Arbitrary-precision decimal type for prices and quantities:

```typescript
const d = Decimal.fromScientific('1.5e2');
const d = Decimal.fromF64(3.14);
const d = Decimal.fromI64(100n);

d.add(other)     d.sub(other)     d.mul(other)     d.div(other)
d.gt(other)      d.lt(other)      d.eq(other)      d.cmp(other)
d.abs()          d.neg()          d.floor(dp)      d.ceil(dp)
d.round(dp)      d.trunc(dp)      d.normalize()    d.fract()
d.min(other)     d.max(other)     d.rem(other)
d.toString()

// Checked arithmetic (returns undefined on overflow)
d.checkedAdd(other)  d.checkedSub(other)  d.checkedMul(other)  d.checkedDiv(other)

// Constants
Decimal.zero()   Decimal.one()
```

### REST API Methods

The client exposes all REST endpoints as typed async methods. Responses are returned as JSON strings.

```typescript
// Market data
await client.exchangeInfo()
await client.tickerPrice(symbol?)
await client.depth(symbol, limit?)
await client.aggTrades(symbol, limit?)
await client.klines(symbol, interval, limit?)

// Account
await client.accountInfo(address)
await client.accountBalance(address)
await client.openOrders(address, symbol?)
await client.allOrders(address, ...)

// System
await client.health()
await client.chainInfo()
```

## Platform Support

| Platform | Import |
|----------|--------|
| Node.js  | `import { Client } from '@bulletxyz/sdk-wasm'` |
| Deno     | `import { Client } from '@bulletxyz/sdk-wasm'` |
| Browser  | `import init, { Client } from '@bulletxyz/sdk-wasm'` then `await init()` |

## License

MIT
