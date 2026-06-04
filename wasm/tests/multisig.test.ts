/**
 * Tests for multisig transaction support.
 *
 * Verifies:
 * 1. MultisigConfig construction, canonicalization, and credential derivation
 * 2. Multisig signable bytes (preamble + V1 JSON payload)
 * 3. Signature collection (validation, threshold, ordering)
 * 4. Wire-format serialization
 * 5. Uniqueness types on the transaction builder (nonce / window / generation)
 */

import { jest } from '@jest/globals';

import {
  Keypair, Transaction, MultisigConfig, SolanaLedgerMultisigTransaction,
  User,
} from "../pkg/node";
import { connectForUserActions } from './helpers';

jest.setTimeout(30_000);

function fixedKeys(): Uint8Array[] {
  const k1 = new Uint8Array(32).fill(1);
  const k2 = new Uint8Array(32).fill(2);
  const k3 = new Uint8Array(32);
  k3[0] = 3;
  // Deliberately out of canonical order
  return [k2, k1, k3];
}

// ── MultisigConfig ───────────────────────────────────────────────────────────

describe('MultisigConfig', () => {
  test('credential id matches the sovereign reference vector', () => {
    const config = new MultisigConfig(2, fixedKeys());

    expect(Buffer.from(config.credentialId()).toString('hex')).toBe(
      '006c5d655c26965616afbf2702bad8096c54723f6571a4230d6bad3b9781645c',
    );
    expect(config.multisigId()).toBe('12eqdPWZ1QKcxSiFBVkcjDgKgHC7RVSqieZNnQUtKhMM');
    expect(config.minSigners()).toBe(2);
  });

  test('canonicalizes pubkeys bytewise', () => {
    const config = new MultisigConfig(2, fixedKeys());
    const keys = config.pubkeys();

    expect(keys[0][0]).toBe(1);
    expect(keys[1][0]).toBe(2);
    expect(keys[2][0]).toBe(3);
  });

  test('rejects invalid configurations', () => {
    const [k1, k2] = fixedKeys();
    expect(() => new MultisigConfig(1, [k1])).toThrow();          // too few signers
    expect(() => new MultisigConfig(0, [k1, k2])).toThrow();      // zero threshold
    expect(() => new MultisigConfig(3, [k1, k2])).toThrow();      // threshold > signers
    expect(() => new MultisigConfig(2, [k1, k1, k2])).toThrow();  // duplicate key
    expect(() => new MultisigConfig(2, [k1, new Uint8Array(31)])).toThrow(); // bad key length
  });
});

// ── SolanaLedgerMultisigTransaction ──────────────────────────────────────────

describe('SolanaLedgerMultisigTransaction', () => {
  async function buildMultisigTx() {
    const client = await connectForUserActions(['CancelAllOrders']);

    const keypairs = [Keypair.generate(), Keypair.generate(), Keypair.generate()];
    const config = new MultisigConfig(2, keypairs.map((kp) => new Uint8Array(kp.publicKey())));

    const unsigned = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .nonce(0n)
      .buildUnsigned(client);

    const tx = new SolanaLedgerMultisigTransaction(unsigned, config);
    return { keypairs, config, tx };
  }

  test('signable bytes carry the multisig preamble and V1 payload', async () => {
    const { config, tx } = await buildMultisigTx();

    const signable = tx.signableBytes();
    // Spec-compliant preamble: 0xff + "solana offchain", signer_count = 3
    expect(signable[0]).toBe(0xff);
    expect(Buffer.from(signable.slice(1, 16)).toString('ascii')).toBe('solana offchain');
    expect(signable[50]).toBe(3);

    const preambleLen = 53 + 3 * 32;
    const json = JSON.parse(Buffer.from(signable.slice(preambleLen)).toString('utf8'));
    expect(json.multisig_id).toBe(config.multisigId());
    expect(json.version).toBe(1);
    expect(json.uniqueness).toEqual({ nonce: 0 });
  });

  test('collects signatures up to the threshold and serializes', async () => {
    const { keypairs, tx } = await buildMultisigTx();

    expect(tx.isComplete()).toBe(false);
    expect(() => tx.toBase64()).toThrow();

    tx.addSignature(
      new Uint8Array(keypairs[0].publicKey()),
      new Uint8Array(keypairs[0].sign(tx.signableBytes())),
    );
    expect(tx.signatureCount()).toBe(1);
    expect(tx.isComplete()).toBe(false);

    tx.addSignature(
      new Uint8Array(keypairs[1].publicKey()),
      new Uint8Array(keypairs[1].sign(tx.signableBytes())),
    );
    expect(tx.signatureCount()).toBe(2);
    expect(tx.isComplete()).toBe(true);

    const b64 = tx.toBase64();
    expect(typeof b64).toBe('string');
    expect(b64.length).toBeGreaterThan(0);
  });

  test('rejects outsiders, duplicates, and invalid signatures', async () => {
    const { keypairs, tx } = await buildMultisigTx();
    const signable = tx.signableBytes();

    const outsider = Keypair.generate();
    expect(() =>
      tx.addSignature(new Uint8Array(outsider.publicKey()), new Uint8Array(outsider.sign(signable))),
    ).toThrow();

    // Invalid signature from a legitimate signer
    expect(() =>
      tx.addSignature(new Uint8Array(keypairs[0].publicKey()), new Uint8Array(64).fill(9)),
    ).toThrow();

    tx.addSignature(
      new Uint8Array(keypairs[0].publicKey()),
      new Uint8Array(keypairs[0].sign(signable)),
    );
    // Duplicate signer
    expect(() =>
      tx.addSignature(
        new Uint8Array(keypairs[0].publicKey()),
        new Uint8Array(keypairs[0].sign(signable)),
      ),
    ).toThrow();
  });
});

// ── Builder uniqueness types ─────────────────────────────────────────────────

describe('Transaction.builder() uniqueness', () => {
  test('nonce and window uniqueness serialize into the message', async () => {
    const client = await connectForUserActions(['CancelAllOrders']);

    const withNonce = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .nonce(7n)
      .buildUnsigned(client);
    const nonceJson = JSON.parse(Buffer.from(withNonce.toMessageBytes()).toString('utf8'));
    expect(nonceJson.uniqueness).toEqual({ nonce: 7 });

    const withWindow = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .window(99n)
      .buildUnsigned(client);
    const windowJson = JSON.parse(Buffer.from(withWindow.toMessageBytes()).toString('utf8'));
    expect(windowJson.uniqueness).toEqual({ window: 99 });
  });

  test('generation and nonce together are rejected', async () => {
    const client = await connectForUserActions(['CancelAllOrders']);

    expect(() =>
      Transaction.builder()
        .callMessage(User.cancelAllOrders())
        .maxFee(10_000_000n)
        .generation(1n)
        .nonce(7n)
        .buildUnsigned(client),
    ).toThrow();
  });
});
