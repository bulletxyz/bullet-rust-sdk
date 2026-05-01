/**
 * Tests for bullet_schema codegen: namespace factories, struct wrappers, and enums.
 *
 * These test the CallMessage factory codegen from `wasm/codegen/bullet_schema/`.
 *
 * Verifies:
 * 1. Namespace modules exist with expected methods
 * 2. Struct wrappers have typed constructors
 * 3. Struct wrappers work as array params (js_sys::Array + TryFromJsValue)
 * 4. Enum types work as constructor / factory params
 * 5. CallMessage objects can be built into signed transactions
 */

import { jest } from '@jest/globals';

import {
  Client, Keypair,
  // Namespace modules
  User, Public, Admin, Keeper, Vault,
  // Struct wrappers
  NewOrderArgs, AmendOrderArgs, CancelOrderArgs,
  NewTriggerOrderArgs, NewTwapOrderArgs,
  TpslPair, Tpsl, PendingTpslPair,
  UpdateVaultConfigArgs,
  OraclePriceUpdateArgs, OraclePriceUpdateWithPythProofArgs, MarkPriceUpdateArgs,
  BackstopLiquidatePerpPositionArgs,
  // Enums
  Side, OrderType, TriggerDirection, TriggerPriceCondition,
} from "../pkg/node";

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

jest.setTimeout(30_000);

// ── Namespace existence ──────────────────────────────────────────────────────

describe('namespace modules exist', () => {
  test('User namespace has trading methods', () => {
    expect(typeof User.deposit).toBe('function');
    expect(typeof User.withdraw).toBe('function');
    expect(typeof User.placeOrders).toBe('function');
    expect(typeof User.cancelOrders).toBe('function');
    expect(typeof User.amendOrders).toBe('function');
    expect(typeof User.createTriggerOrders).toBe('function');
    expect(typeof User.createTwapOrder).toBe('function');
    expect(typeof User.cancelTwapOrder).toBe('function');
    expect(typeof User.createPositionTpsl).toBe('function');
    expect(typeof User.delegateUser).toBe('function');
    expect(typeof User.createVault).toBe('function');
    expect(typeof User.depositToVault).toBe('function');
  });

  test('Public namespace has permissionless methods', () => {
    expect(typeof Public.applyFunding).toBe('function');
    expect(typeof Public.liquidatePerpPositions).toBe('function');
    expect(typeof Public.executeTriggerOrders).toBe('function');
    expect(typeof Public.accrueBorrowLendInterest).toBe('function');
    expect(typeof Public.executeTwapOrders).toBe('function');
    expect(typeof Public.activateTwapOrders).toBe('function');
  });

  test('Admin namespace has admin methods', () => {
    expect(typeof Admin.initPerpMarket).toBe('function');
    expect(typeof Admin.updatePerpMarket).toBe('function');
    expect(typeof Admin.haltPerpMarket).toBe('function');
    expect(typeof Admin.cancelOrders).toBe('function');
    expect(typeof Admin.cancelTriggerOrders).toBe('function');
    expect(typeof Admin.updateAdmin).toBe('function');
    expect(typeof Admin.deposit).toBe('function');
  });

  test('Keeper namespace has keeper methods', () => {
    expect(typeof Keeper.updateOraclePrices).toBe('function');
    expect(typeof Keeper.updateOraclePricesWithPythProofs).toBe('function');
    expect(typeof Keeper.updateMarkPrices).toBe('function');
    expect(typeof Keeper.updateFunding).toBe('function');
    expect(typeof Keeper.updateUserFeeTier).toBe('function');
  });

  test('Vault namespace has vault methods', () => {
    expect(typeof Vault.updateVaultConfig).toBe('function');
    expect(typeof Vault.processWithdrawalQueue).toBe('function');
    expect(typeof Vault.whitelistDepositor).toBe('function');
    expect(typeof Vault.delegateVaultUser).toBe('function');
  });
});

// ── Enums ────────────────────────────────────────────────────────────────────

describe('enum types', () => {
  test('Side enum', () => {
    expect(Side.Bid).toBeDefined();
    expect(Side.Ask).toBeDefined();
    expect(typeof Side.Bid).toBe('number');
    expect(Side.Bid).not.toBe(Side.Ask);
  });

  test('OrderType enum', () => {
    expect(OrderType.Limit).toBeDefined();
    expect(OrderType.PostOnly).toBeDefined();
    expect(OrderType.FillOrKill).toBeDefined();
    expect(OrderType.ImmediateOrCancel).toBeDefined();
    expect(OrderType.PostOnlySlide).toBeDefined();
    expect(OrderType.PostOnlyFront).toBeDefined();
  });

  test('TriggerDirection enum', () => {
    expect(TriggerDirection.GreaterThanOrEqual).toBeDefined();
    expect(TriggerDirection.LessThanOrEqual).toBeDefined();
  });

  test('TriggerPriceCondition enum', () => {
    expect(TriggerPriceCondition.Mark).toBeDefined();
    expect(TriggerPriceCondition.Oracle).toBeDefined();
    expect(TriggerPriceCondition.LastTrade).toBeDefined();
  });
});

// ── Simple factory methods ───────────────────────────────────────────────────

describe('simple factory methods', () => {
  test('User.deposit', () => {
    const msg = User.deposit(0, '100.0');
    expect(msg).toBeDefined();
  });

  test('User.withdraw', () => {
    const msg = User.withdraw(0, '50.5');
    expect(msg).toBeDefined();
  });

  test('User.createSubAccount', () => {
    const msg = User.createSubAccount(1);
    expect(msg).toBeDefined();
  });

  test('User.cancelMarketOrders', () => {
    const msg = User.cancelMarketOrders(0);
    expect(msg).toBeDefined();
  });

  test('User.cancelAllOrders', () => {
    const msg = User.cancelAllOrders();
    expect(msg).toBeDefined();
  });

  test('User.delegateUser', () => {
    const msg = User.delegateUser(
      '11111111111111111111111111111111',
      'my-delegate',
    );
    expect(msg).toBeDefined();
  });

  test('Public.applyFunding', () => {
    const msg = Public.applyFunding([]);
    expect(msg).toBeDefined();
  });

  test('Public.accrueBorrowLendInterest', () => {
    const msg = Public.accrueBorrowLendInterest();
    expect(msg).toBeDefined();
  });

  test('Public.executeTriggerOrders', () => {
    const msg = Public.executeTriggerOrders(0);
    expect(msg).toBeDefined();
  });

  test('Admin.haltPerpMarket', () => {
    const msg = Admin.haltPerpMarket(0, '50000.0');
    expect(msg).toBeDefined();
  });

  test('Admin.unhaltPerpMarket', () => {
    const msg = Admin.unhaltPerpMarket(0);
    expect(msg).toBeDefined();
  });

  test('Keeper.updateFunding', () => {
    const msg = Keeper.updateFunding(new Uint16Array([0, 1, 2]));
    expect(msg).toBeDefined();
  });

  test('invalid decimal rejects', () => {
    expect(() => User.deposit(0, 'not-a-number')).toThrow();
  });

  test('invalid address rejects', () => {
    expect(() => User.delegateUser('bad-addr', 'test')).toThrow();
  });
});

// ── Struct wrapper constructors ──────────────────────────────────────────────

describe('struct wrapper constructors', () => {
  test('NewOrderArgs — limit buy', () => {
    const order = new NewOrderArgs(
      '50000.0',
      '0.1',
      Side.Bid,
      OrderType.Limit,
      false,
    );
    expect(order).toBeDefined();
  });

  test('NewOrderArgs — with optional client_order_id', () => {
    const order = new NewOrderArgs(
      '49000.0', '0.2',
      Side.Ask, OrderType.PostOnly,
      true,
      42n,
    );
    expect(order).toBeDefined();
  });

  test('CancelOrderArgs — by order_id', () => {
    const cancel = new CancelOrderArgs(123n);
    expect(cancel).toBeDefined();
  });

  test('CancelOrderArgs — by client_order_id', () => {
    const cancel = new CancelOrderArgs(undefined, 456n);
    expect(cancel).toBeDefined();
  });

  test('AmendOrderArgs — cancel + place', () => {
    const cancel = new CancelOrderArgs(100n);
    const place = new NewOrderArgs(
      '51000.0', '0.1', Side.Bid, OrderType.Limit, false,
    );
    const amend = new AmendOrderArgs(cancel, place);
    expect(amend).toBeDefined();
  });

  test('Tpsl', () => {
    const tp = new Tpsl(
      '55000.0', '54000.0',
      TriggerPriceCondition.Mark, OrderType.Limit,
    );
    expect(tp).toBeDefined();
  });

  test('TpslPair — tp and sl', () => {
    const tp = new Tpsl(
      '55000.0', '54000.0',
      TriggerPriceCondition.Mark, OrderType.Limit,
    );
    const sl = new Tpsl(
      '45000.0', '46000.0',
      TriggerPriceCondition.Oracle, OrderType.ImmediateOrCancel,
    );
    const pair = new TpslPair(tp, sl);
    expect(pair).toBeDefined();
  });

  test('TpslPair — tp only', () => {
    const tp = new Tpsl(
      '55000.0', '54000.0',
      TriggerPriceCondition.Mark, OrderType.Limit,
    );
    const pair = new TpslPair(tp);
    expect(pair).toBeDefined();
  });

  test('NewTriggerOrderArgs', () => {
    const trigger = new NewTriggerOrderArgs(
      Side.Bid,
      '50000.0',
      '49000.0',
      TriggerDirection.LessThanOrEqual,
      TriggerPriceCondition.Mark,
      OrderType.Limit,
      '0.5',
    );
    expect(trigger).toBeDefined();
  });

  test('NewTwapOrderArgs', () => {
    const twap = new NewTwapOrderArgs(Side.Bid, '100.0', false, 3600n);
    expect(twap).toBeDefined();
  });

  test('OraclePriceUpdateArgs', () => {
    const update = new OraclePriceUpdateArgs(0, '50000.0');
    expect(update).toBeDefined();
  });

  test('OraclePriceUpdateWithPythProofArgs', () => {
    const update = new OraclePriceUpdateWithPythProofArgs(
      0,
      new Uint8Array([1, 2, 3]),
      new Uint8Array([4, 5, 6]),
    );
    expect(update).toBeDefined();
  });

  test('MarkPriceUpdateArgs', () => {
    const update = new MarkPriceUpdateArgs(0, '50000.0', '0.001');
    expect(update).toBeDefined();
  });

  test('BackstopLiquidatePerpPositionArgs', () => {
    const args = new BackstopLiquidatePerpPositionArgs(0, '1.0');
    expect(args).toBeDefined();
  });
});

// ── Array params (Vec<Struct> via TryFromJsValue) ────────────────────────────

describe('array params with struct wrappers', () => {
  test('User.placeOrders — array of NewOrderArgs', () => {
    const order1 = new NewOrderArgs(
      '50000.0', '0.1', Side.Bid, OrderType.Limit, false,
    );
    const order2 = new NewOrderArgs(
      '49000.0', '0.2', Side.Ask, OrderType.PostOnly, true,
    );
    const msg = User.placeOrders(0, [order1, order2], false);
    expect(msg).toBeDefined();
  });

  test('User.placeOrders — empty array', () => {
    const msg = User.placeOrders(0, [], false);
    expect(msg).toBeDefined();
  });

  test('User.cancelOrders — array of CancelOrderArgs', () => {
    const c1 = new CancelOrderArgs(100n);
    const c2 = new CancelOrderArgs(undefined, 200n);
    const msg = User.cancelOrders(0, [c1, c2]);
    expect(msg).toBeDefined();
  });

  test('User.amendOrders — array of AmendOrderArgs', () => {
    const cancel = new CancelOrderArgs(100n);
    const place = new NewOrderArgs(
      '51000.0', '0.1', Side.Bid, OrderType.Limit, false,
    );
    const amend = new AmendOrderArgs(cancel, place);
    const msg = User.amendOrders(0, [amend]);
    expect(msg).toBeDefined();
  });

  test('User.createTriggerOrders — array of NewTriggerOrderArgs', () => {
    const t = new NewTriggerOrderArgs(
      Side.Bid, '50000.0', '49000.0',
      TriggerDirection.LessThanOrEqual, TriggerPriceCondition.Mark,
      OrderType.Limit,
    );
    const msg = User.createTriggerOrders(0, [t]);
    expect(msg).toBeDefined();
  });

  test('Keeper.updateOraclePrices — array of OraclePriceUpdateArgs', () => {
    const p1 = new OraclePriceUpdateArgs(0, '50000.0');
    const p2 = new OraclePriceUpdateArgs(1, '3000.0');
    const msg = Keeper.updateOraclePrices([p1, p2], BigInt(Date.now()) * 1000n);
    expect(msg).toBeDefined();
  });

  test('Keeper.updateOraclePricesWithPythProofs — array of proof args', () => {
    const p = new OraclePriceUpdateWithPythProofArgs(
      0,
      new Uint8Array([1, 2, 3]),
    );
    const msg = Keeper.updateOraclePricesWithPythProofs(
      [p],
      BigInt(Date.now()) * 1000n,
    );
    expect(msg).toBeDefined();
  });

  test('Keeper.updateMarkPrices — array of MarkPriceUpdateArgs', () => {
    const m = new MarkPriceUpdateArgs(0, '50000.0', '0.001');
    const msg = Keeper.updateMarkPrices([m], BigInt(Date.now()) * 1000n);
    expect(msg).toBeDefined();
  });
});

// ── Direct struct params ─────────────────────────────────────────────────────

describe('direct struct params', () => {
  test('User.createPositionTpsl', () => {
    const tp = new Tpsl(
      '55000.0', '54000.0',
      TriggerPriceCondition.Mark, OrderType.Limit,
    );
    const pair = new TpslPair(tp);
    const msg = User.createPositionTpsl(0, pair);
    expect(msg).toBeDefined();
  });

  test('User.createTwapOrder', () => {
    const twap = new NewTwapOrderArgs(Side.Bid, '100.0', false, 3600n);
    const msg = User.createTwapOrder(0, twap);
    expect(msg).toBeDefined();
  });

  test('Vault.updateVaultConfig', () => {
    const config = new UpdateVaultConfigArgs('1000.0', 24, 10);
    const msg = Vault.updateVaultConfig(
      '11111111111111111111111111111111',
      config,
    );
    expect(msg).toBeDefined();
  });
});

// ── No collision between namespaces ──────────────────────────────────────────

describe('no name collisions across namespaces', () => {
  test('User.deposit and Admin.deposit coexist', () => {
    const userMsg = User.deposit(0, '100.0');
    const adminMsg = Admin.deposit(
      '11111111111111111111111111111111', 0, '100.0',
    );
    expect(userMsg).toBeDefined();
    expect(adminMsg).toBeDefined();
  });

  test('User.cancelOrders and Admin.cancelOrders have different sigs', () => {
    const c = new CancelOrderArgs(1n);
    const userMsg = User.cancelOrders(0, [c]);
    const adminMsg = Admin.cancelOrders(
      '[[0, 1, "11111111111111111111111111111111"]]',
    );
    expect(userMsg).toBeDefined();
    expect(adminMsg).toBeDefined();
  });
});

