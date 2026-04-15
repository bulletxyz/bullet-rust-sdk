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
    .userActions(['PlaceOrders', 'CancelOrders'])  // optional schema filter
    .build();

// Metadata
client.chainId()             // u64
client.chainHash()           // Uint8Array (32 bytes)
client.url()                 // REST URL
client.wsUrl()               // WebSocket URL
client.maxFee()              // default max fee
client.hasKeypair()          // whether a default keypair is set

// Submission
await client.sendTransaction(signedTx)  // returns JSON string
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

### External Signing

For hardware wallets or external signing services:

```typescript
// Build unsigned (chain hash is baked in)
const unsigned = Transaction.builder()
    .callMessage(User.deposit(0, '1000.0'))
    .buildUnsigned(client);

// Get signable bytes and sign externally
const signable = unsigned.toBytes();
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

### WebSocket

```typescript
const ws = await client.connectWs();

// Subscribe to topics
await ws.subscribe([
    Topic.aggTrade('BTC-USD'),
    Topic.depth('ETH-USD', OrderbookDepth.D10),
    Topic.bookTicker('SOL-USD'),
    Topic.kline('BTC-USD', KlineInterval.H1),
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
| Browser  | `import init, { Client } from '@bulletxyz/sdk-wasm/pkg/bullet_rust_sdk_wasm.js'` then `await init()` |

## License

MIT
