/**
 * Integration smoke test: connect to mainnet and call exchangeInfo().
 *
 * Runs against the real API – set BULLET_API_ENDPOINT to override the URL.
 */

import { jest } from '@jest/globals';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const { Client } = require('../pkg/node/bullet_rust_sdk_wasm.js');

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

// Network round-trip – give it plenty of time.
jest.setTimeout(30_000);

test('exchangeInfo returns assets and symbols', async () => {
  const client = await Client.connect(ENDPOINT);
  const info = await client.exchangeInfo();

  const assets = info.assets;
  const symbols = info.symbols;

  expect(Array.isArray(assets)).toBe(true);
  expect(assets.length).toBeGreaterThan(0);

  expect(Array.isArray(symbols)).toBe(true);
  expect(symbols.length).toBeGreaterThan(0);

  // Spot-check shape of the first asset and symbol.
  const asset = assets[0];
  expect(typeof asset.asset).toBe('string');
  expect(asset.asset.length).toBeGreaterThan(0);

  const symbol = symbols[0];
  expect(typeof symbol.symbol).toBe('string');
  expect(symbol.symbol.length).toBeGreaterThan(0);
});
