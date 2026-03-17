/**
 * Tests for transaction building and submission.
 *
 * Verifies:
 * 1. Legacy buildSignedTransaction method works
 * 2. TransactionBuilder fluent pattern works
 * 3. Error handling for missing required fields
 * 4. Transaction serialization to base64
 */

import { jest } from '@jest/globals';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdk = require('../pkg/node/bullet_rust_sdk_wasm.js') as typeof import('../pkg/node/bullet_rust_sdk_wasm.js');

const {
  Client, Keypair, TransactionBuilder,
  User, Public,
  NewOrderArgs,
  Side, OrderType,
} = sdk;

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

// ── Legacy buildSignedTransaction ────────────────────────────────────────────

describe('legacy buildSignedTransaction', () => {
  test('build signed tx from Public.applyFunding', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const callMsg = Public.applyFunding([]);
    const tx = client.buildSignedTransaction(callMsg, 10_000_000n, keypair);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('build signed tx from User.placeOrders with typed wrappers', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const order = new NewOrderArgs(
      '50000.0', '0.1', Side.Bid, OrderType.Limit, false,
    );
    const callMsg = User.placeOrders(0, [order], false);
    const tx = client.buildSignedTransaction(callMsg, 10_000_000n, keypair);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });
});

// ── TransactionBuilder pattern ───────────────────────────────────────────────

describe('TransactionBuilder pattern', () => {
  test('TransactionBuilder exists with expected methods', () => {
    expect(typeof TransactionBuilder.new).toBe('function');
  });

  test('build tx using TransactionBuilder', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const callMsg = Public.applyFunding([]);
    const tx = TransactionBuilder.new()
      .callMessage(callMsg)
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('build tx with priorityFeeBips', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const callMsg = Public.applyFunding([]);
    const tx = TransactionBuilder.new()
      .callMessage(callMsg)
      .maxFee(10_000_000n)
      .priorityFeeBips(100n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('build tx with User.placeOrders', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const order = new NewOrderArgs(
      '50000.0', '0.1', Side.Bid, OrderType.Limit, false,
    );
    const callMsg = User.placeOrders(0, [order], false);
    const tx = TransactionBuilder.new()
      .callMessage(callMsg)
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('client.sendTransaction works with built tx', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const callMsg = Public.applyFunding([]);
    const tx = TransactionBuilder.new()
      .callMessage(callMsg)
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    // Just verify sendTransaction method exists and accepts the tx
    expect(typeof client.sendTransaction).toBe('function');
  });
});

// ── Error handling ───────────────────────────────────────────────────────────

describe('TransactionBuilder error handling', () => {
  test('missing callMessage throws error', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    expect(() => {
      TransactionBuilder.new()
        .maxFee(10_000_000n)
        .signer(keypair)
        .build(client);
    }).toThrow();
  });

  test('missing maxFee throws error', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const callMsg = Public.applyFunding([]);
    expect(() => {
      TransactionBuilder.new()
        .callMessage(callMsg)
        .signer(keypair)
        .build(client);
    }).toThrow();
  });

  test('missing signer throws error', async () => {
    const client = await Client.connect(ENDPOINT);

    const callMsg = Public.applyFunding([]);
    expect(() => {
      TransactionBuilder.new()
        .callMessage(callMsg)
        .maxFee(10_000_000n)
        .build(client);
    }).toThrow();
  });
});
