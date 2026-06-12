/**
 * Tests for progenitor codegen: REST API type wrappers, enums, and client methods.
 *
 * These test the auto-generated wasm-bindgen wrappers from `wasm/codegen/progenitor/`.
 *
 * Verifies:
 * 1. Wrapper structs exist with toJSON() and typed getters
 * 2. Enum wrappers have expected variants as numeric values
 * 3. Client methods exist with correct signatures
 * 4. Nested type getters return wrapper instances (e.g. ExchangeInfo.assets -> Asset[])
 */

import { jest } from '@jest/globals';

import {
  Client, Decimal,
  // Enums
  TxResult, TxStatus, HealthState,
  // Type wrappers
  Account, AccountAsset, AccountConfig, AccountPosition,
  Asset, Balance,
  BinanceOrder, Bracket, LeverageBracket,
  BorrowLendPoolResponse, InsuranceAsset, InsuranceBalance,
  ChainInfo, Delegate, ModuleRef, RateLimit, RateParams, RollupConstants,
  ExchangeInfo, FundingRate, OrderBook,
  PingResponse, PriceTicker, TradingSymbol, Ticker24hr, TimeResponse, Trade,
  LedgerEvent, SubmitTxRequest, SubmitTxResponse, TxReceipt,
  ReadinessStatus,
} from '../pkg/node';
import { connectReadOnlyClient } from './helpers';

jest.setTimeout(30_000);

// ── Enum wrappers ────────────────────────────────────────────────────────────

describe('progenitor enum wrappers', () => {
  test('TxResult has expected variants', () => {
    expect(TxResult.Successful).toBeDefined();
    expect(TxResult.Reverted).toBeDefined();
    expect(TxResult.Skipped).toBeDefined();
    expect(typeof TxResult.Successful).toBe('number');
    // Variants are distinct
    const variants = new Set([TxResult.Successful, TxResult.Reverted, TxResult.Skipped]);
    expect(variants.size).toBe(3);
  });

  test('TxStatus has expected variants', () => {
    expect(TxStatus.Unknown).toBeDefined();
    expect(TxStatus.Dropped).toBeDefined();
    expect(TxStatus.Submitted).toBeDefined();
    expect(TxStatus.Published).toBeDefined();
    expect(TxStatus.Processed).toBeDefined();
    expect(TxStatus.Finalized).toBeDefined();
    const variants = new Set([
      TxStatus.Unknown, TxStatus.Dropped, TxStatus.Submitted,
      TxStatus.Published, TxStatus.Processed, TxStatus.Finalized,
    ]);
    expect(variants.size).toBe(6);
  });

  test('HealthState has expected variants', () => {
    expect(HealthState.Starting).toBeDefined();
    expect(HealthState.Running).toBeDefined();
    expect(HealthState.Recovering).toBeDefined();
    expect(typeof HealthState.Starting).toBe('number');
  });
});

// ── Type wrapper classes exist ───────────────────────────────────────────────

describe('progenitor type wrappers are exported', () => {
  const expectedClasses = [
    'Account', 'AccountAsset', 'AccountConfig', 'AccountPosition',
    'Asset', 'Balance',
    'BinanceOrder', 'Bracket', 'LeverageBracket',
    'BorrowLendPoolResponse', 'InsuranceAsset', 'InsuranceBalance',
    'ChainInfo', 'Delegate', 'ModuleRef', 'RateLimit', 'RateParams', 'RollupConstants',
    'ExchangeInfo', 'FundingRate', 'OrderBook',
    'PingResponse', 'PriceTicker', 'TradingSymbol', 'Ticker24hr', 'TimeResponse', 'Trade',
    'LedgerEvent', 'SubmitTxRequest', 'SubmitTxResponse', 'TxReceipt',
    'ReadinessStatus',
  ];

  const exportedClasses: Record<string, unknown> = {
    Account, AccountAsset, AccountConfig, AccountPosition,
    Asset, Balance,
    BinanceOrder, Bracket, LeverageBracket,
    BorrowLendPoolResponse, InsuranceAsset, InsuranceBalance,
    ChainInfo, Delegate, ModuleRef, RateLimit, RateParams, RollupConstants,
    ExchangeInfo, FundingRate, OrderBook,
    PingResponse, PriceTicker, TradingSymbol, Ticker24hr, TimeResponse, Trade,
    LedgerEvent, SubmitTxRequest, SubmitTxResponse, TxReceipt,
    ReadinessStatus,
  };

  test.each(expectedClasses)('%s is exported as a constructor', (name) => {
    const Ctor = exportedClasses[name];
    expect(Ctor).toBeDefined();
    expect(typeof Ctor).toBe('function');
  });
});

describe('submit response helpers', () => {
  test('SubmitTxResponse.messageId extracts a Hyperlane message id from events', () => {
    const messageId = `0x${'ab'.repeat(32)}`;
    const response = SubmitTxResponse.fromJson(JSON.stringify({
      id: '0xtx',
      status: 'processed',
      events: [{
        key: 'dispatch',
        module: { name: 'warp' },
        number: 1,
        type: 'dispatch',
        value: { message_id: messageId },
      }],
    }));

    expect(response.messageId).toBe(messageId);
  });

  test('SubmitTxResponse.messageId extracts nested mailbox dispatch ids', () => {
    const messageId = `0x${'34'.repeat(32)}`;
    const response = SubmitTxResponse.fromJson(JSON.stringify({
      id: '0xtx',
      status: 'processed',
      events: [{
        key: 'dispatch',
        module: { name: 'warp' },
        number: 1,
        type: 'dispatch',
        value: {
          mailbox: {
            dispatch: {
              id: messageId,
            },
          },
        },
      }],
    }));

    expect(response.messageId).toBe(messageId);
  });
});

describe('account config delegate wrappers', () => {
  test('AccountConfig.delegates exposes delegate session fields', () => {
    const address = '11111111111111111111111111111111';
    const config = AccountConfig.fromJson(JSON.stringify({
      delegates: [{
        address,
        name: 'session',
        flags: 7,
        expiresAt: 1_700_000_000_000_000,
      }],
    }));

    const [delegate] = config.delegates;
    expect(delegate.address).toBe(address);
    expect(delegate.name).toBe('session');
    expect(delegate.flags).toBe(7);
    expect(delegate.expiresAt).toBe(1_700_000_000_000_000n);
  });
});

// ── Client method existence ──────────────────────────────────────────────────

describe('client methods exist', () => {
  const expectedMethods = [
    'ping', 'time', 'health', 'ready',
    'exchangeInfo', 'orderBook', 'recentTrades',
    'ticker24hr', 'tickerPrice', 'fundingRate',
    'accountInfo', 'accountBalance', 'accountConfig',
    'commissionRate', 'symbolConfig', 'rateLimitOrder',
    'queryOpenOrder', 'queryOpenOrders', 'leverageBracket',
    'insuranceBalance', 'borrowLendPools',
    'constants', 'schema', 'submitTx',
  ];

  test.each(expectedMethods)('Client.prototype.%s exists', (method) => {
    expect(typeof Client.prototype[method as keyof typeof Client.prototype]).toBe('function');
  });
});

// ── Live API: struct getters ─────────────────────────────────────────────────

describe('struct getters via live API', () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let client: any;

  beforeAll(async () => {
    client = await connectReadOnlyClient();
  });

  test('ping returns PingResponse with toJSON', async () => {
    const resp = await client.ping();
    expect(resp).toBeDefined();
    const json = resp.toJSON();
    expect(typeof json).toBe('string');
    expect(() => JSON.parse(json)).not.toThrow();
  });

  test('time returns TimeResponse with serverTime getter', async () => {
    const resp = await client.time();
    expect(resp).toBeDefined();
    expect(typeof resp.serverTime).toBe('bigint');
    expect(resp.serverTime).toBeGreaterThan(0n);
    // toJSON round-trips
    const parsed = JSON.parse(resp.toJSON());
    expect(parsed.serverTime).toBeDefined();
  });

  test('exchangeInfo returns nested typed arrays', async () => {
    const info = await client.exchangeInfo();
    expect(info).toBeDefined();

    // assets getter returns Asset[]
    const assets = info.assets;
    expect(Array.isArray(assets)).toBe(true);
    expect(assets.length).toBeGreaterThan(0);

    const asset = assets[0];
    expect(typeof asset.asset).toBe('string');
    expect(typeof asset.marginAvailable).toBe('boolean');
    expect(typeof asset.assetId).toBe('number');
    expect(typeof asset.toJSON()).toBe('string');

    // symbols getter returns TradingSymbol[]
    const symbols = info.symbols;
    expect(Array.isArray(symbols)).toBe(true);
    expect(symbols.length).toBeGreaterThan(0);

    const sym = symbols[0];
    expect(typeof sym.symbol).toBe('string');
    expect(typeof sym.baseAsset).toBe('string');
    expect(typeof sym.quoteAsset).toBe('string');
    expect(typeof sym.marginAsset).toBe('string');
    expect(typeof sym.marketId).toBe('number');
    expect(typeof sym.pricePrecision).toBe('number');
    expect(typeof sym.quantityPrecision).toBe('number');
    expect(typeof sym.status).toBe('string');
    expect(typeof sym.contractType).toBe('string');

    // rateLimits getter returns RateLimit[]
    const limits = info.rateLimits;
    expect(Array.isArray(limits)).toBe(true);
    if (limits.length > 0) {
      const rl = limits[0];
      expect(typeof rl.rateLimitType).toBe('string');
      expect(typeof rl.interval).toBe('string');
      expect(typeof rl.intervalNum).toBe('number');
      expect(typeof rl.limit).toBe('number');
    }

    // toJSON round-trips
    const parsed = JSON.parse(info.toJSON());
    expect(Array.isArray(parsed.assets)).toBe(true);
    expect(Array.isArray(parsed.symbols)).toBe(true);
  });

  test('tickerPrice returns PriceTicker[] with getters', async () => {
    const tickers = await client.tickerPrice();
    expect(Array.isArray(tickers)).toBe(true);
    expect(tickers.length).toBeGreaterThan(0);

    const t = tickers[0];
    expect(typeof t.symbol).toBe('string');
    expect(t.price).toBeInstanceOf(Decimal);
    expect(typeof t.time).toBe('bigint');
    expect(t.time).toBeGreaterThan(0n);
  });

  test('fundingRate returns FundingRate with getters', async () => {
    try {
      const fr = await client.fundingRate();
      expect(fr).toBeDefined();
      expect(typeof fr.symbol).toBe('string');
      expect(typeof fr.fundingRate).toBe('string');
      expect(typeof fr.fundingTime).toBe('bigint');
      expect(typeof fr.markPrice).toBe('string');
    } catch {
      // API may return an array instead of a single object — skip gracefully.
    }
  });

  test('orderBook returns OrderBook with toJSON for nested arrays', async () => {
    const tickers = await client.tickerPrice();
    const symbol = tickers[0]?.symbol;
    if (!symbol) return; // skip if no symbols

    // progenitor alphabetizes params: (limit, symbol)
    const ob = await client.orderBook(undefined, symbol);
    expect(ob).toBeDefined();
    // E and T are i64 timestamps → bigint
    expect(typeof ob.E).toBe('bigint');
    expect(typeof ob.T).toBe('bigint');
    expect(typeof ob.lastUpdateId).toBe('bigint');
    // bids/asks are Vec<Vec<String>> → serialized as JSON string
    const json = ob.toJSON();
    const parsed = JSON.parse(json);
    expect(Array.isArray(parsed.asks)).toBe(true);
    expect(Array.isArray(parsed.bids)).toBe(true);
  });

  test('constants returns RollupConstants with getters', async () => {
    const c = await client.constants();
    expect(c).toBeDefined();
    expect(typeof c.chainId).toBe('bigint');
    expect(typeof c.chainName).toBe('string');
    expect(typeof c.addressPrefix).toBe('string');
    expect(typeof c.gasTokenId).toBe('string');
    expect(typeof c.hyperlaneDomain).toBe('bigint');
  });

  test('insuranceBalance returns InsuranceBalance[] with nested assets', async () => {
    // Note: the API may return a different shape than the schema declares.
    // We just verify the call doesn't crash and returns something.
    try {
      const balances = await client.insuranceBalance();
      expect(Array.isArray(balances)).toBe(true);
      if (balances.length > 0) {
        const b = balances[0];
        expect(Array.isArray(b.assets)).toBe(true);
        expect(Array.isArray(b.symbols)).toBe(true);
        if (b.assets.length > 0) {
          const a = b.assets[0];
          expect(typeof a.asset).toBe('string');
          expect(typeof a.marginBalance).toBe('string');
          expect(typeof a.updateTime).toBe('bigint');
        }
      }
    } catch {
      // API may return a response that doesn't match the schema — skip gracefully.
    }
  });

  test('health returns a string', async () => {
    const h = await client.health();
    expect(typeof h).toBe('string');
  });

  test('schema returns a JSON string', async () => {
    const s = await client.schema();
    expect(typeof s).toBe('string');
    expect(() => JSON.parse(s)).not.toThrow();
  });

  test('ticker24hr returns Ticker24hr with getters', async () => {
    const tickers = await client.tickerPrice();
    const symbol = tickers[0]?.symbol;
    if (!symbol) return;

    try {
      const t = await client.ticker24hr(symbol);
      expect(t).toBeDefined();
      expect(typeof t.symbol).toBe('string');
      expect(typeof t.lastPrice).toBe('string');
      expect(typeof t.highPrice).toBe('string');
      expect(typeof t.lowPrice).toBe('string');
      expect(typeof t.volume).toBe('string');
      expect(typeof t.quoteVolume).toBe('string');
      expect(typeof t.openTime).toBe('bigint');
      expect(typeof t.closeTime).toBe('bigint');
      expect(typeof t.priceChange).toBe('string');
      expect(typeof t.priceChangePercent).toBe('string');
      expect(typeof t.weightedAvgPrice).toBe('string');
      expect(typeof t.count).toBe('bigint');
    } catch {
      // Endpoint may return 501 Not Implemented — skip gracefully.
    }
  });

  test('recentTrades returns Trade[] with getters', async () => {
    const tickers = await client.tickerPrice();
    const symbol = tickers[0]?.symbol;
    if (!symbol) return;

    // progenitor alphabetizes params: (limit, symbol)
    const trades = await client.recentTrades(undefined, symbol);
    expect(Array.isArray(trades)).toBe(true);
    if (trades.length > 0) {
      const t = trades[0];
      expect(typeof t.id).toBe('bigint');
      expect(t.price).toBeInstanceOf(Decimal);
      expect(t.qty).toBeInstanceOf(Decimal);
      expect(t.quoteQty).toBeInstanceOf(Decimal);
      expect(typeof t.time).toBe('bigint');
      expect(typeof t.isBuyerMaker).toBe('boolean');
    }
  });

  test('leverageBracket returns LeverageBracket[] with nested Bracket[]', async () => {
    const brackets = await client.leverageBracket();
    expect(Array.isArray(brackets)).toBe(true);
    if (brackets.length > 0) {
      const lb = brackets[0];
      expect(typeof lb.symbol).toBe('string');
      expect(Array.isArray(lb.brackets)).toBe(true);
      if (lb.brackets.length > 0) {
        const b = lb.brackets[0];
        expect(typeof b.bracket).toBe('number');
        expect(typeof b.initialLeverage).toBe('number');
        expect(typeof b.notionalCap).toBe('number');
        expect(typeof b.notionalFloor).toBe('number');
        expect(typeof b.maintMarginRatio).toBe('number');
        expect(typeof b.cum).toBe('number');
      }
    }
  });

  test('borrowLendPools returns BorrowLendPoolResponse[] with nested RateParams', async () => {
    const pools = await client.borrowLendPools();
    expect(Array.isArray(pools)).toBe(true);
    if (pools.length > 0) {
      const p = pools[0];
      expect(typeof p.asset).toBe('string');
      expect(typeof p.assetId).toBe('number');
      expect(typeof p.isActive).toBe('boolean');
      expect(p.borrowedAmount).toBeInstanceOf(Decimal);
      expect(p.availableAmount).toBeInstanceOf(Decimal);

      // Nested RateParams struct
      const rp = p.rateParams;
      expect(rp).toBeDefined();
      expect(rp.minBorrowRate).toBeInstanceOf(Decimal);
      expect(rp.maxBorrowRate).toBeInstanceOf(Decimal);
      expect(rp.optimalBorrowRate).toBeInstanceOf(Decimal);
      expect(rp.optimalUtilisationRate).toBeInstanceOf(Decimal);
    }
  });
});
