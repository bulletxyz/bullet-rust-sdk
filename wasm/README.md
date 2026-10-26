# @bulletxyz/sdk-wasm

WebAssembly bindings for the [Bullet](https://bullet.xyz) trading API. Works in Node.js, browsers, and Deno.

## Install

```bash
npm install @bulletxyz/sdk-wasm
```

## Quick Start

```typescript
import { Client, Keypair, Transaction, User, NewOrderArgs, Side, OrderType } from '@bulletxyz/sdk-wasm';

const keypair = Keypair.fromHex('your-private-key');
const client = await Client.builder()
    .network('mainnet')
    .keypair(keypair)
    .build();

const marketId = client.marketId('BTC-USD');
if (marketId === undefined) throw new Error('unknown market');
const order = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false);
const response = await Transaction.builder()
    .callMessage(User.placeOrders(marketId, [order], false))
    .send(client);
```

For market data without a signer, use `await Client.mainnet()` and typed REST
methods such as `client.tickerPrice('BTC-USD')`.

### Degen trading

Use the typed `DegenAction` factories to open or close a degen position. The
action is passed directly into the generated call message without JSON
serialization.

`open` executes all-or-nothing at top-of-book: `Side.Bid` opens a long and
`Side.Ask` opens a short. The size must be non-zero and aligned to the market's
lot size. `amountToTransfer` funds the isolated balance from cross margin and is
capped by the protocol at 10 USDC. `close` executes IOC and returns the remaining
isolated balance to cross margin once the position is flat.

```typescript
import { DegenAction, Side, User } from '@bulletxyz/sdk-wasm';

const open = User.degenTrade(
    7,
    DegenAction.open('5', Side.Bid, '10'),
);

const close = User.degenTrade(7, DegenAction.close(), 2);
```

## API Reference

### Client

Use `Client.mainnet()`, `Client.connect(networkOrUrl)`, or the builder shown above.
The installed package's TypeScript declarations describe all methods and parameters.

By default the client validates every exchange call-message group against the
server schema at connection. `.userActions(['PlaceOrders', 'CancelOrders'])`
limits validation to those user actions and rejects other actions before signing.
Builder settings supply signing defaults; transaction builders can override them.

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
    .callMessage(User.cancelAllOrders())
    .signer(keypair)
    .send(client);

// Build without sending
const tx = Transaction.builder()
    .callMessage(msg)
    .signer(keypair)
    .build(client);

const submitResponse = await client.sendTransaction(tx);
submitResponse.id       // transaction hash
submitResponse.status   // 'submitted' | 'processed' | ...
submitResponse.events   // LedgerEvent[]
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

### Executing as another account

By default a transaction executes as the signer's own default address. Use
`.addressOverride(address)` to execute as an account that explicitly authorised
the signer's credential (e.g. a multisig or a delegated account). The override
is signed as part of the transaction and included in both the Borsh payload
(`toBytes()`) and the Solana offchain JSON (`toMessageBytes()`). It takes a
base58 address and throws on an invalid one.

```typescript
const tx = Transaction.builder()
    .callMessage(msg)
    .addressOverride(multisigAddress) // base58 string
    .signer(memberKeypair)
    .build(client);
```

### External Signing

For hardware wallets or external signing services that can sign the standard
Borsh payload:

```typescript
// Build unsigned (chain hash is baked in)
const unsigned = Transaction.builder()
    .callMessage(User.cancelAllOrders())
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
unsigned.toBytes()  // Uint8Array — borsh signing payload: version byte, tx, chain hash (signable bytes)
unsigned.toDisplayMessage() // string — human-readable unsigned payload for display only
unsigned.toMessageBytes() // Uint8Array — readable JSON bytes for Solana wallets

// Inverse of toBytes(): rebuild an UnsignedTransaction from stored signable
// bytes (chain name comes from the client; mismatched chain hash is rejected).
UnsignedTransaction.fromBytes(bytes, client) // UnsignedTransaction
```

Use `fromBytes` when a coordinator (e.g. a multisig UI) persists a transaction
as its exact signable bytes and rebuilds it later for display and submission —
the rebuilt bytes are byte-identical to what was signed, so the stored bytes are
the source of truth rather than a separate JSON representation.

`fromBytes` only accepts bytes produced by this signing format. Bytes persisted
by a pre-fork SDK — which had no leading version byte — cannot be rebuilt and
are rejected with a serialization error. The network may still admit a
transaction signed over them while the environment's legacy cutoff is open
(single-signer transactions only), but rebuild those transactions from their
inputs and re-sign them to be safe.

Some external wallets display `signMessage` bytes as raw UTF-8, so `toBytes()`
can look garbled in the wallet confirmation. That is expected: those bytes are
what the network verifies. Use `toDisplayMessage()` in your app UI to show the
transaction contents before asking the wallet to sign `toBytes()`.

For external Solana wallets where the wallet confirmation should show readable
JSON, use the Solana offchain path instead. `toMessageBytes()` returns the
readable JSON payload with `chain_name`, `details.chain_hash_fragment` and
`version` as the domain fields.
`SolanaOffchainTransaction.fromParts(...)` also carries the chain hash required
by the current sequencer offchain authenticator envelope:

```typescript
const unsigned = Transaction.builder()
    .callMessage(User.cancelAllOrders())
    .buildUnsigned(client);

const message = unsigned.toMessageBytes();
const signature = await wallet.signMessage(message);
const pubKey = wallet.publicKey.toBytes(); // 32-byte Solana public key
const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);

await client.sendOffChainTransaction(tx);
```

### Signed transactions

`Transaction.builder().build(client)` and `SignedTransaction.fromParts(...)`
produce signed transactions. `toBytes()` serializes the Borsh envelope;
`toBase64()` encodes it for WebSocket submission.

`SolanaOffchainTransaction.fromParts(...)` wraps a signature over
`unsigned.toMessageBytes()`. Submit it with `client.sendOffChainTransaction(tx)`;
its `toBytes()` and `toBase64()` serialize the offchain envelope.

### Multisig

M-of-N multisig over the spec-compliant Solana offchain format (the format
Ledger hardware wallets sign). Every signer signs the same bytes; once the
threshold is met the transaction can be submitted.

```typescript
// 2-of-3 multisig. Keys are canonicalized (sorted) internally, so input
// order never matters.
const config = new MultisigConfig(2, [pubkeyA, pubkeyB, pubkeyC]);

const unsigned = Transaction.builder()
    .callMessage(User.cancelAllOrders())
    .buildUnsigned(client);

// Collect signatures — each signer signs the same signable bytes
const tx = new SolanaLedgerMultisigTransaction(unsigned, config);
const signature = await ledgerWallet.signMessage(tx.signableBytes());
tx.addSignature(pubkeyA, signature);   // validates membership + signature
// ... pass tx.signableBytes() to the other signers ...
tx.addSignature(pubkeyB, signatureB);

// Submit (throws if below threshold)
await client.sendLedgerMultisigTransaction(tx);
```

The signable bytes can also be produced without assembling a transaction:
`unsigned.toLedgerMultisigSignableBytes(config)` (preamble + JSON) and
`unsigned.toMultisigMessageBytes(config)` (JSON payload only).

### Keypair

Use `Keypair.generate()`, `Keypair.fromHex(secret)` or `Keypair.fromBytes(bytes)`.
The secret is 32 bytes; `publicKey()` returns the 32-byte Ed25519 public key and
`sign(bytes)` returns a 64-byte signature.

### CallMessage Factories

Generated namespaces (`User`, `Public`, `Keeper`, `Vault`, `Admin`, `Warp`)
construct call messages. Permissionless actions still require a signed transaction.
Use the installed TypeScript declarations for factory signatures.

Use `User.transfer` for balance movement. `TransferEndpoint` takes a JSON-encoded
balance bucket and an optional sub-account index:

```typescript
import { TransferEndpoint, User } from '@bulletxyz/sdk-wasm';

const deposit = User.transfer(
    new TransferEndpoint(JSON.stringify('bank')),
    new TransferEndpoint(JSON.stringify('cross')),
    0, '1000.0', '',
);
```

The older `User.deposit`, `User.withdraw` and `User.createSubAccount` factories
remain for wire compatibility, but the exchange rejects those actions.

Delegate expiry is encoded as epoch microseconds. If your app stores epoch
milliseconds, pass `BigInt(expiresAtMs) * 1000n` for `expiresAt`.

Read configured delegates with `accountConfig`:

```typescript
const config = await client.accountConfig(address);

for (const delegate of config.delegates) {
    delegate.address
    delegate.name
    delegate.flags
    delegate.expiresAt  // bigint | undefined, epoch microseconds
}
```

`deriveVaultAddress(name)` computes a vault's address from its name. See
[`examples/node/create_vault.ts`](../examples/node/create_vault.ts) for a runnable
vault example.

`User.cancelQueuedWithdrawalV1(vault, withdrawalTimestamp?)` cancels the queued
withdrawal whose `timestamp` (from `queuedWithdrawals` on `vault`/`vaults`)
matches, or all of the sender's queued withdrawals from the vault when the
timestamp is omitted. The older `User.cancelQueuedWithdrawal(vault)` cancels
only the oldest one.

`deriveSubAccountAddress(master, index)` computes a sub-account's address from
its master address and index (`0-31`). A transfer into a sub-account creates it;
pass the index to the destination `TransferEndpoint`. To trade there, pass the
index as the trailing `sub_account_index` argument on order/cancel factories.
Use the derived address for reads such as `client.accountInfo(sub)`.
Out-of-range indices throw.

### Warp Bridge Withdrawals

Bridge withdrawals use a normal typed call message and the same transaction
builder path as exchange user actions:

```typescript
import { Transaction, Warp } from '@bulletxyz/sdk-wasm';

const callMsg = Warp.transferRemote({
    warpRoute: '0x...',          // bytes32 route
    amount: '1000000',
    destinationDomain: 1234,
    gasPaymentLimit: '400000',
    recipient: '0x...',          // bytes32; 20-byte EVM hex is left-padded
    relayer: null,               // or a string / { Standard: string } / { Vm: string }
});

const unsigned = Transaction.builder()
    .callMessage(callMsg)
    .buildUnsigned(client);
```

Use decimal strings for `amount` and `gasPaymentLimit` values above
`Number.MAX_SAFE_INTEGER`; unsafe JavaScript numbers are rejected to avoid
rounding token amounts before signing.

After submission, bridge responses expose the Hyperlane message id when the
Trading API includes it in transaction events:

```typescript
const response = await client.sendOffChainTransaction(tx);
response.messageId  // string | undefined
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

### Managed WebSocket (auto-reconnect)

For long-running bots, prefer `connectWsManaged` — it handles reconnection with
exponential backoff and replays your subscriptions automatically. Sends are queued;
acknowledgements arrive as `message` events. Calls throw when the command queue
is full or the background task has stopped.

Protocol ping/pong keeps the raw socket alive but does not reset managed idle
detection. For quiet subscriptions, send application pings, catch timer errors,
and clear the interval when the receive loop ends. Pass a `ManagedWsConfig` to
customize reconnect and idle settings; its parameters are in the type declarations:

```typescript
const ws = await client.connectWsManaged();

ws.subscribe([Topic.depth('BTC-USD', OrderbookDepth.D20)]);

const keepalive = setInterval(() => {
    try {
        ws.ping();
    } catch (err) {
        console.error('keepalive ping failed:', err);
    }
}, 30_000);

try {
    while (true) {
        const evt = await ws.recv();
        if (!evt) break;
        switch (evt.type) {
            case 'message':       handleMessage(evt.message); break;
            case 'reconnecting':  console.log('reconnecting...'); break;
            case 'disconnected':  console.error(evt.reason); return;
        }
    }
} finally {
    clearInterval(keepalive);
}
```

### Decimal

Fixed-precision decimal arithmetic for prices and quantities. Prefer string
inputs such as `Decimal.fromScientific('1.5e2')` to preserve decimal precision.
Checked arithmetic (`checkedAdd`, `checkedSub`, `checkedMul`, `checkedDiv`)
returns `undefined` on failure.

### REST API Methods

The client exposes REST endpoints as typed async methods. Response objects have
typed getters plus `toJSON()` for full JSON serialization.

Arguments are generated in alphabetical order, not in "required first" order, so
a required argument can follow a nullable one — pass `null` or `undefined` for
the ones you do not need (for example `client.orderBook(null, 'BTC-USD')`).
Arguments marked `?` may be omitted entirely.

`vaultEquity(address)` returns the user's active positions across vaults plus
`totalEquity`. Each position includes `vaultAddress`, `shares`, `equity`, and a
nullable `percentageReturn`.

Each item returned by `vaultDepositors` includes held, queued, and total shares
plus equity including the main vault account and its vault-owned sub-accounts.
All pages and pricing come from one committed rollup state version, reported as
`stateSlot`. `stateUpdatedAt` is when that state was collected (Unix
microseconds, refreshed every 30 seconds by default), not the block time.
`vaultEquity` returns HTTP 503 if the last successful collection is over 90
seconds old; `vaultDepositors` keeps serving last-good rows.

On `vaultDepositors`, history fields (`initialDepositAmount`,
`initialDepositTimestamp`, `remainingCostBasis`, `historyUpdatedAt`,
`historyLastTxNumber`, `historyShares`) come from an optional historical
projection and are null (or undefined in the bindings) when unavailable; stale
history does not block current shares or equity. `historyComplete` means the
projection has complete, valid history, not that it has caught up with the live
position.

`percentageReturn` must be displayed as an **estimated unrealized return**. On
`vaultDepositors`, `percentageReturnIsEstimate` is true whenever it is non-null.
`percentageReturn` is null when history is older than 30 minutes, live and
historical shares differ, remaining cost basis is zero, or arithmetic overflows.
On `vaultDepositors`, it is also null when the state collection is older than 90
seconds; `vaultEquity` returns HTTP 503 instead. It is not lifetime cash-flow
return and excludes realized withdrawals.

## Platform Support

| Platform | Import |
|----------|--------|
| Node.js  | `import { Client } from '@bulletxyz/sdk-wasm'` |
| Deno     | `import { Client } from '@bulletxyz/sdk-wasm'` |
| Browser  | `import init, { Client } from '@bulletxyz/sdk-wasm'` then `await init()` |

## License

MIT
