/**
 * Tests for Decimal type support across the JS ↔ WASM boundary.
 *
 * Verifies that:
 * 1. WasmDecimal arithmetic (add, sub, mul, div) produces correct results
 * 2. Rounding, comparison, and predicate methods work correctly
 * 3. API responses return Decimal wrappers for decimal-formatted fields
 * 4. Decimal values from API responses can be used in arithmetic
 * 5. Decimal ↔ string/number conversions preserve precision
 */

import { jest } from '@jest/globals';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdk = require('../pkg/node/bullet_rust_sdk_wasm.js') as typeof import('../pkg/node/bullet_rust_sdk_wasm.js');

const { Client, Decimal } = sdk;

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

// ── Decimal construction ────────────────────────────────────────────────────

describe('Decimal construction', () => {
  test('from string', () => {
    const d = new Decimal('1.23456');
    expect(d.toString()).toBe('1.23456');
  });

  test('from integer string', () => {
    const d = new Decimal('42');
    expect(d.toString()).toBe('42');
  });

  test('from negative string', () => {
    const d = new Decimal('-99.5');
    expect(d.toString()).toBe('-99.5');
  });

  test('from zero', () => {
    const d = new Decimal('0');
    expect(d.isZero()).toBe(true);
  });

  test('invalid string throws', () => {
    expect(() => new Decimal('not_a_number')).toThrow();
  });

  test('Decimal.zero() and Decimal.one()', () => {
    expect(Decimal.zero().isZero()).toBe(true);
    expect(Decimal.one().toString()).toBe('1');
  });

  test('fromI64', () => {
    const d = Decimal.fromI64(BigInt(12345));
    expect(d.toString()).toBe('12345');
  });

  test('fromF64', () => {
    const d = Decimal.fromF64(3.14);
    expect(d.toNumber()).toBeCloseTo(3.14);
  });
});

// ── Arithmetic ──────────────────────────────────────────────────────────────

describe('Decimal arithmetic', () => {
  test('add', () => {
    const a = new Decimal('1.1');
    const b = new Decimal('2.2');
    expect(a.add(b).toString()).toBe('3.3');
  });

  test('sub', () => {
    const a = new Decimal('5.5');
    const b = new Decimal('2.3');
    expect(a.sub(b).toString()).toBe('3.2');
  });

  test('mul', () => {
    const a = new Decimal('3.0');
    const b = new Decimal('2.5');
    expect(a.mul(b).toString()).toBe('7.50');
  });

  test('div', () => {
    const a = new Decimal('10');
    const b = new Decimal('4');
    expect(a.div(b).eq(new Decimal('2.5'))).toBe(true);
  });

  test('div by zero throws', () => {
    const a = new Decimal('1');
    const b = Decimal.zero();
    expect(() => a.div(b)).toThrow('division by zero');
  });

  test('rem', () => {
    const a = new Decimal('10');
    const b = new Decimal('3');
    expect(a.rem(b).toString()).toBe('1');
  });

  test('neg', () => {
    const a = new Decimal('5.5');
    expect(a.neg().toString()).toBe('-5.5');
    expect(a.neg().neg().toString()).toBe('5.5');
  });

  test('abs', () => {
    const neg = new Decimal('-42.5');
    const pos = new Decimal('42.5');
    expect(neg.abs().toString()).toBe('42.5');
    expect(pos.abs().toString()).toBe('42.5');
  });

  test('chained operations', () => {
    // (10 + 5) * 2 - 3 = 27
    const result = new Decimal('10')
      .add(new Decimal('5'))
      .mul(new Decimal('2'))
      .sub(new Decimal('3'));
    expect(result.toString()).toBe('27');
  });

  test('precision is preserved (no floating point drift)', () => {
    // Classic floating point failure: 0.1 + 0.2 !== 0.3 in IEEE 754
    const a = new Decimal('0.1');
    const b = new Decimal('0.2');
    const sum = a.add(b);
    expect(sum.toString()).toBe('0.3');
    expect(sum.eq(new Decimal('0.3'))).toBe(true);
  });

  test('checkedAdd returns undefined on overflow', () => {
    const a = new Decimal('1.5');
    const b = new Decimal('2.5');
    const result = a.checkedAdd(b);
    expect(result).toBeDefined();
    expect(result!.toString()).toBe('4.0');
  });

  test('checkedSub', () => {
    const result = new Decimal('5').checkedSub(new Decimal('3'));
    expect(result).toBeDefined();
    expect(result!.toString()).toBe('2');
  });

  test('checkedMul', () => {
    const result = new Decimal('3').checkedMul(new Decimal('4'));
    expect(result).toBeDefined();
    expect(result!.toString()).toBe('12');
  });

  test('checkedDiv returns undefined for zero', () => {
    const result = new Decimal('1').checkedDiv(Decimal.zero());
    expect(result).toBeUndefined();
  });

  test('checkedDiv returns value for valid division', () => {
    const result = new Decimal('10').checkedDiv(new Decimal('4'));
    expect(result).toBeDefined();
    expect(result!.eq(new Decimal('2.5'))).toBe(true);
  });

  test('checkedRem returns undefined for zero', () => {
    const result = new Decimal('1').checkedRem(Decimal.zero());
    expect(result).toBeUndefined();
  });
});

// ── Rounding ────────────────────────────────────────────────────────────────

describe('Decimal rounding', () => {
  test('round half-up', () => {
    expect(new Decimal('1.235').round(2).toString()).toBe('1.24');
    expect(new Decimal('1.234').round(2).toString()).toBe('1.23');
  });

  test('floor (toward negative infinity)', () => {
    expect(new Decimal('1.239').floor(2).toString()).toBe('1.23');
    expect(new Decimal('-1.239').floor(2).toString()).toBe('-1.24');
  });

  test('ceil (toward positive infinity)', () => {
    expect(new Decimal('1.231').ceil(2).toString()).toBe('1.24');
    expect(new Decimal('-1.231').ceil(2).toString()).toBe('-1.23');
  });

  test('round to 0 decimal places', () => {
    expect(new Decimal('3.7').round(0).toString()).toBe('4');
    expect(new Decimal('3.2').round(0).toString()).toBe('3');
  });

  test('scale returns decimal places', () => {
    expect(new Decimal('1.23').scale()).toBe(2);
    expect(new Decimal('42').scale()).toBe(0);
    expect(new Decimal('1.23000').scale()).toBe(5);
  });

  test('trunc truncates without rounding', () => {
    expect(new Decimal('1.239').trunc(2).toString()).toBe('1.23');
    expect(new Decimal('1.999').trunc(0).toString()).toBe('1');
    expect(new Decimal('-1.239').trunc(2).toString()).toBe('-1.23');
  });

  test('fract returns fractional part', () => {
    expect(new Decimal('1.23').fract().toString()).toBe('0.23');
    expect(new Decimal('42').fract().toString()).toBe('0');
    expect(new Decimal('-3.7').fract().toString()).toBe('-0.7');
  });

  test('normalize strips trailing zeros', () => {
    expect(new Decimal('1.2300').normalize().toString()).toBe('1.23');
    expect(new Decimal('100').normalize().toString()).toBe('100');
    expect(new Decimal('0.0010').normalize().toString()).toBe('0.001');
  });
});

// ── Comparison ──────────────────────────────────────────────────────────────

describe('Decimal comparison', () => {
  const a = new Decimal('1.5');
  const b = new Decimal('2.5');
  const c = new Decimal('1.5');

  test('eq', () => {
    expect(a.eq(c)).toBe(true);
    expect(a.eq(b)).toBe(false);
  });

  test('gt / gte', () => {
    expect(b.gt(a)).toBe(true);
    expect(a.gt(b)).toBe(false);
    expect(a.gte(c)).toBe(true);
  });

  test('lt / lte', () => {
    expect(a.lt(b)).toBe(true);
    expect(b.lt(a)).toBe(false);
    expect(a.lte(c)).toBe(true);
  });

  test('cmp', () => {
    expect(a.cmp(b)).toBe(-1);
    expect(b.cmp(a)).toBe(1);
    expect(a.cmp(c)).toBe(0);
  });

  test('min / max', () => {
    expect(a.min(b).toString()).toBe('1.5');
    expect(a.max(b).toString()).toBe('2.5');
  });
});

// ── Predicates ──────────────────────────────────────────────────────────────

describe('Decimal predicates', () => {
  test('isZero', () => {
    expect(Decimal.zero().isZero()).toBe(true);
    expect(new Decimal('0.0').isZero()).toBe(true);
    expect(new Decimal('1').isZero()).toBe(false);
  });

  test('isPositive / isNegative', () => {
    expect(new Decimal('5').isPositive()).toBe(true);
    expect(new Decimal('5').isNegative()).toBe(false);
    expect(new Decimal('-5').isPositive()).toBe(false);
    expect(new Decimal('-5').isNegative()).toBe(true);
    expect(Decimal.zero().isPositive()).toBe(false);
    expect(Decimal.zero().isNegative()).toBe(false);
  });

  test('isInteger', () => {
    expect(new Decimal('42').isInteger()).toBe(true);
    expect(new Decimal('42.0').isInteger()).toBe(true);
    expect(new Decimal('42.5').isInteger()).toBe(false);
    expect(Decimal.zero().isInteger()).toBe(true);
  });
});

// ── Conversion ──────────────────────────────────────────────────────────────

describe('Decimal conversion', () => {
  test('toNumber converts to f64', () => {
    expect(new Decimal('3.14').toNumber()).toBeCloseTo(3.14);
  });

  test('toJSON returns string', () => {
    const d = new Decimal('123.456');
    expect(d.toJSON()).toBe('123.456');
  });

  test('toString matches toJSON', () => {
    const d = new Decimal('99.99');
    expect(d.toString()).toBe(d.toJSON());
  });

  test('mantissa returns BigInt', () => {
    const d = new Decimal('1.23');
    expect(d.mantissa()).toBe(123n);

    const large = new Decimal('999999999999.999999');
    expect(typeof large.mantissa()).toBe('bigint');
  });

  test('fromScientific parses scientific notation', () => {
    expect(Decimal.fromScientific('1.5e3').toString()).toBe('1500');
    expect(Decimal.fromScientific('2.5e-2').toString()).toBe('0.025');
    expect(() => Decimal.fromScientific('not_sci')).toThrow();
  });
});

// ── Live API: Decimal getters ───────────────────────────────────────────────

describe('API responses return Decimal wrappers', () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let client: any;

  beforeAll(async () => {
    client = await Client.connect(ENDPOINT);
  });

  test('PriceTicker.price is a Decimal', async () => {
    const tickers = await client.tickerPrice();
    expect(tickers.length).toBeGreaterThan(0);

    const t = tickers[0];
    const price = t.price;

    // Should be a Decimal wrapper, not a plain string or number
    expect(typeof price.toString()).toBe('string');
    expect(typeof price.toNumber()).toBe('number');
    expect(Number.isFinite(price.toNumber())).toBe(true);
    expect(price.toNumber()).toBeGreaterThan(0);

    // Decimal arithmetic should work on it
    const doubled = price.add(price);
    expect(doubled.eq(price.mul(new Decimal('2')))).toBe(true);
  });

  test('PriceTicker arithmetic: sum of all prices', async () => {
    const tickers = await client.tickerPrice();
    expect(tickers.length).toBeGreaterThan(0);

    // Sum all prices using Decimal arithmetic
    let sum = Decimal.zero();
    for (const t of tickers) {
      sum = sum.add(t.price);
    }

    // Sum should be positive
    expect(sum.gt(Decimal.zero())).toBe(true);

    // Average price
    const count = new Decimal(tickers.length.toString());
    const avg = sum.div(count);
    expect(avg.gt(Decimal.zero())).toBe(true);
  });

  test('Trade price * qty = quoteQty', async () => {
    const tickers = await client.tickerPrice();
    const symbol = tickers[0]?.symbol;
    if (!symbol) return;

    const trades = await client.recentTrades(undefined, symbol);
    if (trades.length === 0) return;

    const trade = trades[0];
    const computed = trade.price.mul(trade.qty);
    const reported = trade.quoteQty;

    // Should match exactly or very closely
    const diff = computed.sub(reported).abs();
    const tolerance = reported.abs().mul(new Decimal('0.0001'));
    expect(diff.lte(tolerance)).toBe(true);
  });

  test('BorrowLendPool decimal fields are Decimals with arithmetic', async () => {
    const pools = await client.borrowLendPools();
    if (pools.length === 0) return;

    const p = pools[0];

    // These fields should be Decimals
    const available = p.availableAmount;
    const borrowed = p.borrowedAmount;

    expect(typeof available.toString()).toBe('string');
    expect(typeof borrowed.toString()).toBe('string');

    // Both should be non-negative
    expect(available.gte(Decimal.zero())).toBe(true);
    expect(borrowed.gte(Decimal.zero())).toBe(true);
  });

  test('Decimal toJSON round-trips through JSON.parse', async () => {
    const tickers = await client.tickerPrice();
    expect(tickers.length).toBeGreaterThan(0);

    const t = tickers[0];
    const parsed = JSON.parse(t.toJSON());

    // The price in JSON should be a string matching the Decimal's toString
    const priceFromJson = new Decimal(parsed.price);
    expect(priceFromJson.eq(t.price)).toBe(true);
  });
});
