/**
 * Integration test: build, sign, and submit a transaction.
 *
 * Uses a permissionless ApplyFunding call message so no funded account is
 * needed. The transaction will be accepted by the sequencer (we get back a
 * tx hash and status) even though it may be reverted on-chain.
 *
 * Set BULLET_API_ENDPOINT to override the target.
 */

import { jest } from '@jest/globals';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const { Client, Keypair, CallMessage } = require('../pkg/node/bullet_rust_sdk_wasm.js');

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

test('build, sign, and submit a transaction', async () => {
  const client = await Client.connect(ENDPOINT);
  const keypair = Keypair.generate();

  // ApplyFunding is permissionless — no account balance required.
  const callMsg = CallMessage.applyFunding([]);

  const tx = client.buildSignedTransaction(callMsg, 10_000_000n, keypair);

  // The opaque Transaction should expose toBase64 for WebSocket use.
  const b64 = tx.toBase64();
  expect(typeof b64).toBe('string');
  expect(b64.length).toBeGreaterThan(0);

  // Submit via REST — the sequencer accepts the tx and returns an id + status.
  const responseJson = await client.submitTransaction(tx);
  const response = JSON.parse(responseJson);

  expect(typeof response.id).toBe('string');
  expect(response.id.length).toBeGreaterThan(0);
  expect(typeof response.status).toBe('string');
});
