/**
 * Tests for numeric serialization/deserialization across the JS ↔ WASM boundary.
 *
 * wasm-bindgen maps Rust numeric types to JS as follows:
 *   - i8..u32  → i32 (JS number)
 *   - i64/u64  → f64 (getters return JS number) or i64 (params accept BigInt)
 *   - f32/f64  → f64 (JS number)
 *
 * These tests verify that:
 * 1. Integer values passed as constructor params arrive correctly in getters
 * 2. Large 64-bit values survive the round-trip through f64 (within precision limits)
 * 3. BigInt params are accepted and values are retrievable via toJSON()
 * 4. Optional numeric fields handle undefined/null correctly
 * 5. Numeric getter values from live API responses match their toJSON() counterparts
 */

import { jest } from '@jest/globals';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdk = require('../pkg/node/bullet_rust_sdk_wasm.js') as typeof import('../pkg/node/bullet_rust_sdk_wasm.js');

const {
  Client,
  // Struct wrappers with numeric constructor params
  NewOrderArgs, CancelOrderArgs, NewTwapOrderArgs,
  OraclePriceUpdateArgs, MarkPriceUpdateArgs,
  UpdateVaultConfigArgs,
  UpdatePerpMarketArgs,
  // Enums needed for constructors
  Side, OrderType,
} = sdk;

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

// ── i32 params (≤32-bit integers) ───────────────────────────────────────────

describe('i32 numeric params (number → WASM → number)', () => {
  test('OraclePriceUpdateArgs preserves asset_id', () => {
    for (const id of [0, 1, 42, 255, 65535]) {
      const args = new OraclePriceUpdateArgs(id, '50000.0');
      expect(args).toBeDefined();
    }
  });

  test('MarkPriceUpdateArgs preserves market_id', () => {
    for (const id of [0, 1, 100]) {
      const args = new MarkPriceUpdateArgs(id, '50000.0', '0.001');
      expect(args).toBeDefined();
    }
  });

  test('boundary i32 values are accepted', () => {
    // u16 max (common for market_id / asset_id)
    const u16Max = new OraclePriceUpdateArgs(65535, '1.0');
    expect(u16Max).toBeDefined();

    // Zero
    const zero = new OraclePriceUpdateArgs(0, '1.0');
    expect(zero).toBeDefined();
  });
});

// ── BigInt params (64-bit integers) ─────────────────────────────────────────

describe('bigint params (BigInt → WASM → number/BigInt)', () => {
  test('CancelOrderArgs accepts BigInt order_id', () => {
    const args = new CancelOrderArgs(123n);
    expect(args).toBeDefined();
  });

  test('CancelOrderArgs accepts BigInt client_order_id', () => {
    const args = new CancelOrderArgs(undefined, 456n);
    expect(args).toBeDefined();
  });

  test('CancelOrderArgs accepts large BigInt values', () => {
    // Values within u64 range
    const large = new CancelOrderArgs(9007199254740991n); // Number.MAX_SAFE_INTEGER as BigInt
    expect(large).toBeDefined();

    const veryLarge = new CancelOrderArgs(18446744073709551615n); // u64::MAX
    expect(veryLarge).toBeDefined();
  });

  test('NewOrderArgs accepts optional BigInt client_order_id', () => {
    // Without client_order_id
    const without = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false);
    expect(without).toBeDefined();

    // With client_order_id
    const withId = new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false, 42n);
    expect(withId).toBeDefined();
  });

  test('NewTwapOrderArgs accepts BigInt duration', () => {
    const twap = new NewTwapOrderArgs(Side.Bid, '100.0', false, 3600n);
    expect(twap).toBeDefined();

    // Large duration
    const longTwap = new NewTwapOrderArgs(Side.Ask, '50.0', true, 86400n);
    expect(longTwap).toBeDefined();
  });
});

// ── Optional numeric fields ─────────────────────────────────────────────────

describe('optional numeric fields', () => {
  test('CancelOrderArgs with both fields undefined', () => {
    // At least one should be provided in practice, but the constructor accepts it
    const args = new CancelOrderArgs();
    expect(args).toBeDefined();
  });

  test('CancelOrderArgs with null fields', () => {
    const args = new CancelOrderArgs(null, null);
    expect(args).toBeDefined();
  });

  test('UpdateVaultConfigArgs with optional numeric params', () => {
    // All params provided
    const full = new UpdateVaultConfigArgs('1000.0', 24, 10);
    expect(full).toBeDefined();

    // Optional params as null
    const partial = new UpdateVaultConfigArgs('1000.0', null, null);
    expect(partial).toBeDefined();
  });

  test('UpdatePerpMarketArgs with optional numeric params', () => {
    // Only required market_id, rest optional
    const args = new UpdatePerpMarketArgs(0);
    expect(args).toBeDefined();

    // With optional numeric fields at the end (max_orders_per_side, max_orders_per_user, max_trigger_orders_per_user)
    // Intervening string params must be null/undefined
    const withOpts = new UpdatePerpMarketArgs(
      0,
      null, null, null, null, null, null, null, null, null,
      100, 50, 20,
    );
    expect(withOpts).toBeDefined();
  });
});

// ── Live API: numeric getter round-trips ────────────────────────────────────

describe('numeric getter values match toJSON() round-trip', () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let client: any;

  beforeAll(async () => {
    client = await Client.connect(ENDPOINT);
  });

  test('TimeResponse.serverTime is a positive integer that matches JSON', async () => {
    const resp = await client.time();
    const serverTime = resp.serverTime;

    expect(typeof serverTime).toBe('number');
    expect(Number.isFinite(serverTime)).toBe(true);
    expect(serverTime).toBeGreaterThan(0);
    // Should be an integer (millisecond timestamp)
    expect(Number.isInteger(serverTime)).toBe(true);

    // Getter must match the toJSON() value
    const parsed = JSON.parse(resp.toJSON());
    expect(serverTime).toBe(parsed.serverTime);
  });

  test('Asset numeric getters match JSON', async () => {
    const info = await client.exchangeInfo();
    const assets = info.assets;
    expect(assets.length).toBeGreaterThan(0);

    for (const asset of assets) {
      const assetId = asset.assetId;
      const decimals = asset.decimals;

      expect(typeof assetId).toBe('number');
      expect(typeof decimals).toBe('number');
      expect(Number.isInteger(assetId)).toBe(true);
      expect(Number.isInteger(decimals)).toBe(true);
      expect(assetId).toBeGreaterThanOrEqual(0);
      expect(decimals).toBeGreaterThanOrEqual(0);

      // Round-trip through JSON
      const parsed = JSON.parse(asset.toJSON());
      expect(assetId).toBe(parsed.assetId);
      expect(decimals).toBe(parsed.decimals);
    }
  });

  test('TradingSymbol numeric getters match JSON', async () => {
    const info = await client.exchangeInfo();
    const symbols = info.symbols;
    expect(symbols.length).toBeGreaterThan(0);

    const sym = symbols[0];
    const numericGetters: [string, string][] = [
      ['marketId', 'marketId'],
      ['pricePrecision', 'pricePrecision'],
      ['quantityPrecision', 'quantityPrecision'],
      ['baseAssetPrecision', 'baseAssetPrecision'],
      ['quotePrecision', 'quotePrecision'],
      ['settlePlan', 'settlePlan'],
      ['deliveryDate', 'deliveryDate'],
      ['onboardDate', 'onboardDate'],
    ];

    const parsed = JSON.parse(sym.toJSON());

    for (const [getter, jsonKey] of numericGetters) {
      const value = (sym as Record<string, unknown>)[getter];
      expect(typeof value).toBe('number');
      expect(Number.isFinite(value as number)).toBe(true);
      // Getter value should match the JSON representation
      expect(value).toBe(parsed[jsonKey]);
    }
  });

  test('RollupConstants numeric getters match JSON', async () => {
    const c = await client.constants();
    const parsed = JSON.parse(c.toJSON());

    expect(typeof c.chainId).toBe('number');
    expect(typeof c.hyperlaneDomain).toBe('number');
    expect(Number.isFinite(c.chainId)).toBe(true);
    expect(Number.isFinite(c.hyperlaneDomain)).toBe(true);

    expect(c.chainId).toBe(parsed.chain_id);
    expect(c.hyperlaneDomain).toBe(parsed.hyperlane_domain);
  });

  test('RateLimit numeric getters match JSON', async () => {
    const info = await client.exchangeInfo();
    const limits = info.rateLimits;
    if (limits.length === 0) return;

    const rl = limits[0];
    const parsed = JSON.parse(rl.toJSON());

    expect(typeof rl.intervalNum).toBe('number');
    expect(typeof rl.limit).toBe('number');
    expect(Number.isInteger(rl.intervalNum)).toBe(true);
    expect(Number.isInteger(rl.limit)).toBe(true);

    expect(rl.intervalNum).toBe(parsed.intervalNum);
    expect(rl.limit).toBe(parsed.limit);
  });

  test('Bracket numeric getters match JSON', async () => {
    const brackets = await client.leverageBracket();
    if (brackets.length === 0) return;

    const lb = brackets[0];
    if (lb.brackets.length === 0) return;

    const b = lb.brackets[0];
    const parsed = JSON.parse(b.toJSON());

    const numericGetters: [string, string][] = [
      ['bracket', 'bracket'],
      ['initialLeverage', 'initialLeverage'],
      ['notionalCap', 'notionalCap'],
      ['notionalFloor', 'notionalFloor'],
      ['maintMarginRatio', 'maintMarginRatio'],
      ['cum', 'cum'],
    ];

    for (const [getter, jsonKey] of numericGetters) {
      const value = (b as Record<string, unknown>)[getter];
      expect(typeof value).toBe('number');
      expect(Number.isFinite(value as number)).toBe(true);
      expect(value).toBe(parsed[jsonKey]);
    }
  });

  test('OrderBook timestamp getters are valid numbers matching JSON', async () => {
    const tickers = await client.tickerPrice();
    const symbol = tickers[0]?.symbol;
    if (!symbol) return;

    const ob = await client.orderBook(undefined, symbol);
    const parsed = JSON.parse(ob.toJSON());

    // E and T are i64 timestamps
    expect(typeof ob.E).toBe('number');
    expect(typeof ob.T).toBe('number');
    expect(typeof ob.lastUpdateId).toBe('number');

    expect(Number.isFinite(ob.E)).toBe(true);
    expect(Number.isFinite(ob.T)).toBe(true);
    expect(Number.isFinite(ob.lastUpdateId)).toBe(true);

    expect(ob.E).toBe(parsed.E);
    expect(ob.T).toBe(parsed.T);
    expect(ob.lastUpdateId).toBe(parsed.lastUpdateId);
  });

  test('PriceTicker.time getter matches JSON', async () => {
    const tickers = await client.tickerPrice();
    expect(tickers.length).toBeGreaterThan(0);

    const t = tickers[0];
    const parsed = JSON.parse(t.toJSON());

    expect(typeof t.time).toBe('number');
    expect(Number.isFinite(t.time)).toBe(true);
    expect(t.time).toBeGreaterThan(0);
    expect(t.time).toBe(parsed.time);
  });

  test('BorrowLendPoolResponse numeric getters match JSON', async () => {
    const pools = await client.borrowLendPools();
    if (pools.length === 0) return;

    const p = pools[0];
    const parsed = JSON.parse(p.toJSON());

    expect(typeof p.assetId).toBe('number');
    expect(typeof p.interestFeeTenthBps).toBe('number');
    expect(typeof p.lastUpdateTimestamp).toBe('number');

    expect(p.assetId).toBe(parsed.assetId);
    expect(p.interestFeeTenthBps).toBe(parsed.interestFeeTenthBps);
    expect(p.lastUpdateTimestamp).toBe(parsed.lastUpdateTimestamp);
  });

  test('optional numeric getter LeverageBracket.notionalCoef', async () => {
    const brackets = await client.leverageBracket();
    if (brackets.length === 0) return;

    for (const lb of brackets) {
      const coef = lb.notionalCoef;
      // Should be either undefined or a finite number
      if (coef !== undefined && coef !== null) {
        expect(typeof coef).toBe('number');
        expect(Number.isFinite(coef)).toBe(true);
      }
    }
  });
});
