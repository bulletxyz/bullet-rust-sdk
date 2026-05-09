/**
 * Integration smoke test: connect to mainnet and call exchangeInfo().
 */

import { jest } from '@jest/globals';

import { connectReadOnlyClient } from './helpers';

jest.setTimeout(30_000);

test('exchangeInfo returns assets and symbols', async () => {
  const client = await connectReadOnlyClient();
  const info = await client.exchangeInfo();

  const assets = info.assets;
  const symbols = info.symbols;

  expect(Array.isArray(assets)).toBe(true);
  expect(assets.length).toBeGreaterThan(0);

  expect(Array.isArray(symbols)).toBe(true);
  expect(symbols.length).toBeGreaterThan(0);

  const asset = assets[0];
  expect(typeof asset.asset).toBe('string');
  expect(asset.asset.length).toBeGreaterThan(0);

  const symbol = symbols[0];
  expect(typeof symbol.symbol).toBe('string');
  expect(symbol.symbol.length).toBeGreaterThan(0);
});
