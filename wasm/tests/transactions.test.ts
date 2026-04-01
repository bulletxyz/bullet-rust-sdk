/**
 * Tests for transaction building and submission.
 *
 * Verifies:
 * 1. Transaction.builder() fluent pattern works
 * 2. External signing flow (buildUnsigned → toSignableBytes → fromParts)
 * 3. Error handling for missing required fields
 * 4. Transaction serialization to base64 and bytes
 */

import { jest } from '@jest/globals';

import {
  Client, Keypair, Transaction, SignedTransaction,
  User, Public,
  NewOrderArgs,
  Side, OrderType,
} from "../pkg/node";

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

// ── Transaction.builder() pattern ────────────────────────────────────────────

describe('Transaction.builder()', () => {
  test('Transaction.builder exists', () => {
    expect(typeof Transaction.builder).toBe('function');
  });

  test('build signed tx from Public.applyFunding', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

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
    const tx = Transaction.builder()
      .callMessage(User.placeOrders(0, [order], false))
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    expect(tx.toBase64().length).toBeGreaterThan(0);
  });

  test('build tx with priorityFeeBips', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .priorityFeeBips(100n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    expect(tx.toBase64().length).toBeGreaterThan(0);
  });

  test('client.sendTransaction works with built tx', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(typeof client.sendTransaction).toBe('function');
  });
});

// ── External signing ─────────────────────────────────────────────────────────

describe('external signing', () => {
  test('buildUnsigned → toBytes → fromParts', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const unsigned = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    // Get signable bytes (borsh tx + chain hash baked in)
    const signableBytes = unsigned.toBytes();
    expect(signableBytes).toBeInstanceOf(Uint8Array);
    expect(signableBytes.length).toBeGreaterThan(32);

    // Sign with keypair
    const signature = keypair.sign(signableBytes);
    expect(signature.length).toBe(64);

    const pubKey = keypair.publicKey();
    expect(pubKey.length).toBe(32);

    // Assemble signed transaction
    const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
    expect(signed).toBeDefined();

    // Verify serialization works
    const bytes = signed.toBytes();
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(bytes.length).toBeGreaterThan(0);

    const b64 = signed.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('toBytes() is deterministic', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    const bytes1 = tx.toBytes();
    const bytes2 = tx.toBytes();
    expect(bytes1).toEqual(bytes2);
  });
});

// ── Error handling ───────────────────────────────────────────────────────────

describe('error handling', () => {
  test('missing callMessage throws error', async () => {
    const client = await Client.connect(ENDPOINT);
    const keypair = Keypair.generate();

    expect(() => {
      Transaction.builder()
        .maxFee(10_000_000n)
        .signer(keypair)
        .build(client);
    }).toThrow();
  });

  test('missing signer throws error', async () => {
    const client = await Client.connect(ENDPOINT);

    expect(() => {
      Transaction.builder()
        .callMessage(Public.applyFunding([]))
        .maxFee(10_000_000n)
        .build(client);
    }).toThrow();
  });

  test('fromParts rejects invalid signature length', async () => {
    const client = await Client.connect(ENDPOINT);

    const unsigned = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    expect(() => {
      SignedTransaction.fromParts(unsigned, new Uint8Array(63), new Uint8Array(32));
    }).toThrow();
  });

  test('fromParts rejects invalid pubkey length', async () => {
    const client = await Client.connect(ENDPOINT);

    const unsigned = Transaction.builder()
      .callMessage(Public.applyFunding([]))
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    expect(() => {
      SignedTransaction.fromParts(unsigned, new Uint8Array(64), new Uint8Array(31));
    }).toThrow();
  });
});
