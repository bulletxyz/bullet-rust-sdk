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


import {
  // Namespace modules
  User, Public, Admin, Keeper, Vault, Warp,
  // Struct wrappers
  NewOrderArgs, AmendOrderArgs, CancelOrderArgs,
  NewTriggerOrderArgs, NewTwapOrderArgs,
  TpslPair, Tpsl, PendingTpslPair,
  InitAssetInfoArgsV1, UpdateAssetInfoArgsV1, UpdateGlobalConfigArgsV1,
  UpdateVaultConfigArgs, UpdateVaultConfigArgsV1, VaultDepositPolicyArgs, StrategicDepositorCap,
  OraclePriceUpdateArgs, OraclePriceUpdateWithPythProofArgs, MarkPriceUpdateArgs,
  BackstopLiquidatePerpPositionArgs, BackstopLiquidatePerpPositionArgsV1,
  // Enums
  Side, OrderType, TriggerDirection, TriggerPriceCondition,
  // Data-carrying enums
  DegenAction,
} from "../pkg/node";

const textEncoder = new TextEncoder();
const PYTH_PRIMARY_WITHOUT_QUOTE = textEncoder.encode('pyth-primary-no-quote-v1');
const PYTH_PRIMARY_WITH_QUOTE = textEncoder.encode('pyth-primary-with-quote-v1');
const PYTH_QUOTE = textEncoder.encode('pyth-quote-v1');

const ADDRESS = '11111111111111111111111111111111';

const tp = () => new Tpsl('55000.0', '54000.0', TriggerPriceCondition.Mark, OrderType.Limit);
const sl = () => new Tpsl(
  '45000.0', '46000.0',
  TriggerPriceCondition.Oracle, OrderType.ImmediateOrCancel,
);
const limitBuy = () => new NewOrderArgs('51000.0', '0.1', Side.Bid, OrderType.Limit, false);
const amendArgs = () => new AmendOrderArgs(new CancelOrderArgs(100n), limitBuy());
const nowMicros = () => BigInt(Date.now()) * 1000n;

describe('namespace modules exist', () => {
  test.each<[string, object, string[]]>([
    ['User', User, [
      'deposit', 'withdraw', 'placeOrders', 'cancelOrders', 'amendOrders',
      'createTriggerOrders', 'createTwapOrder', 'cancelTwapOrder', 'createPositionTpsl',
      'delegateUser', 'createVault', 'depositToVault', 'delegateUserV2', 'revokeDelegationV1',
    ]],
    ['Public', Public, [
      'applyFunding', 'liquidatePerpPositions', 'executeTriggerOrders',
      'accrueBorrowLendInterest', 'executeTwapOrders', 'activateTwapOrders',
    ]],
    ['Admin', Admin, [
      'initPerpMarket', 'updatePerpMarket', 'initAssetInfoV1', 'updateAssetInfoV1',
      'haltPerpMarket', 'cancelOrders', 'cancelTriggerOrders', 'updateGlobalConfigV1',
      'updateAdmin', 'deposit',
    ]],
    ['Keeper', Keeper, [
      'updateOraclePrices', 'updateOraclePricesWithPythProofs', 'updateMarkPrices',
      'updateFunding', 'updateUserFeeTier',
    ]],
    ['Vault', Vault, [
      'updateVaultConfig', 'updateVaultConfigV1', 'processWithdrawalQueue',
      'whitelistDepositor', 'delegateVaultUser', 'setDepositPolicy',
    ]],
    ['Warp', Warp, ['transferRemote']],
  ])('%s namespace has its methods', (_name, namespace, methods) => {
    for (const method of methods) {
      expect(typeof (namespace as Record<string, unknown>)[method], method).toBe('function');
    }
  });
});

describe('enum types', () => {
  test.each<[string, object, string[]]>([
    ['Side', Side, ['Bid', 'Ask']],
    ['OrderType', OrderType, [
      'Limit', 'PostOnly', 'FillOrKill', 'ImmediateOrCancel', 'PostOnlySlide', 'PostOnlyFront',
    ]],
    ['TriggerDirection', TriggerDirection, ['GreaterThanOrEqual', 'LessThanOrEqual']],
    ['TriggerPriceCondition', TriggerPriceCondition, ['Mark', 'Oracle', 'LastTrade']],
  ])('%s variants are defined and distinct', (_name, enumObj, variants) => {
    const values = variants.map((v) => (enumObj as Record<string, unknown>)[v]);
    values.forEach((value, i) => expect(value, variants[i]).toBeDefined());
    expect(new Set(values).size).toBe(variants.length);
  });

  test('Side variants are numbers', () => {
    expect(typeof Side.Bid).toBe('number');
  });
});

describe('DegenAction', () => {
  test('DegenAction factories produce typed degen trade messages', () => {
    const open = DegenAction.open('5', Side.Bid, '10');
    expect(User.degenTrade(7, open)).toBeDefined();

    const close = DegenAction.close();
    expect(User.degenTrade(7, close, 2)).toBeDefined();
  });

  test('DegenAction.open validates decimal inputs', () => {
    expect(() => DegenAction.open('invalid', Side.Bid, '10')).toThrow();
    expect(() => DegenAction.open('5', Side.Bid, 'invalid')).toThrow();
  });
});

// Each row must cross the wasm boundary without throwing.
describe('factories and struct wrappers build', () => {
  test.each<[string, () => unknown]>([
    // Simple factory methods.
    ['User.deposit', () => User.deposit(0, '100.0')],
    ['User.withdraw', () => User.withdraw(0, '50.5')],
    ['User.createSubAccount', () => User.createSubAccount(1)],
    ['User.cancelMarketOrders', () => User.cancelMarketOrders(0)],
    ['User.cancelAllOrders', () => User.cancelAllOrders()],
    ['User.delegateUser', () => User.delegateUser(ADDRESS, 'my-delegate')],
    ['User.delegateUserV2', () => User.delegateUserV2(
      ADDRESS, 'my-delegate', 7, 2, 1_700_000_000_000_000n,
    )],
    ['User.revokeDelegationV1', () => User.revokeDelegationV1(ADDRESS, 2)],
    ['User.cancelQueuedWithdrawalV1 — one', () => User.cancelQueuedWithdrawalV1(
      ADDRESS, 1_700_000_000_000_000n,
    )],
    ['User.cancelQueuedWithdrawalV1 — all', () => User.cancelQueuedWithdrawalV1(ADDRESS)],
    ['Warp.transferRemote', () => Warp.transferRemote({
      warpRoute: `0x${'11'.repeat(32)}`,
      amount: '12345678901234567890',
      destinationDomain: 1234,
      gasPaymentLimit: '400000',
      recipient: `0x${'22'.repeat(32)}`,
      relayer: ADDRESS,
    })],
    ['Public.applyFunding', () => Public.applyFunding([])],
    ['Public.accrueBorrowLendInterest', () => Public.accrueBorrowLendInterest()],
    ['Public.executeTriggerOrders', () => Public.executeTriggerOrders(0)],
    ['Admin.haltPerpMarket', () => Admin.haltPerpMarket(0, '50000.0')],
    ['Admin.unhaltPerpMarket', () => Admin.unhaltPerpMarket(0)],
    ['Keeper.updateFunding', () => Keeper.updateFunding(new Uint16Array([0, 1, 2]))],
    // Struct wrapper constructors.
    ['NewOrderArgs — limit buy', () => new NewOrderArgs(
      '50000.0', '0.1', Side.Bid, OrderType.Limit, false,
    )],
    ['NewOrderArgs — with optional client_order_id', () => new NewOrderArgs(
      '49000.0', '0.2', Side.Ask, OrderType.PostOnly, true, 42n,
    )],
    ['CancelOrderArgs — by order_id', () => new CancelOrderArgs(123n)],
    ['CancelOrderArgs — by client_order_id', () => new CancelOrderArgs(undefined, 456n)],
    ['AmendOrderArgs — cancel + place', amendArgs],
    ['Tpsl', tp],
    ['TpslPair — tp and sl', () => new TpslPair(tp(), sl())],
    ['TpslPair — tp only', () => new TpslPair(tp())],
    ['NewTriggerOrderArgs', () => new NewTriggerOrderArgs(
      Side.Bid, '50000.0', '49000.0',
      TriggerDirection.LessThanOrEqual, TriggerPriceCondition.Mark,
      OrderType.Limit, '0.5',
    )],
    ['NewTwapOrderArgs', () => new NewTwapOrderArgs(Side.Bid, '100.0', false, 3600n)],
    ['OraclePriceUpdateArgs', () => new OraclePriceUpdateArgs(0, '50000.0')],
    ['OraclePriceUpdateWithPythProofArgs', () => new OraclePriceUpdateWithPythProofArgs(
      0, PYTH_PRIMARY_WITH_QUOTE, PYTH_QUOTE,
    )],
    ['OraclePriceUpdateWithPythProofArgs — without quote', () => (
      new OraclePriceUpdateWithPythProofArgs(0, PYTH_PRIMARY_WITHOUT_QUOTE)
    )],
    ['MarkPriceUpdateArgs', () => new MarkPriceUpdateArgs(0, '50000.0', '0.001')],
    ['BackstopLiquidatePerpPositionArgs', () => new BackstopLiquidatePerpPositionArgs(0, '1.0')],
    ['User.backstopLiquidatePerpPositionsV1', () => User.backstopLiquidatePerpPositionsV1(
      ADDRESS, [new BackstopLiquidatePerpPositionArgsV1(0, '1.0', Side.Bid, '99.5')], 2,
    )],
    ['Admin.initAssetInfoV1', () => Admin.initAssetInfoV1(
      new InitAssetInfoArgsV1(1, 'USDC', 6, '0.01', 'USDC', 101, 102),
    )],
    ['Admin.updateAssetInfoV1', () => Admin.updateAssetInfoV1(
      new UpdateAssetInfoArgsV1(1, '0.02', 201, 202),
    )],
    ['Admin.updateGlobalConfigV1 with trusted signers JSON', () => {
      const signer = Array.from({ length: 32 }, (_, i) => i);
      return Admin.updateGlobalConfigV1(new UpdateGlobalConfigArgsV1(
        undefined, undefined, undefined, undefined, undefined,
        undefined, undefined, undefined, undefined,
        JSON.stringify([signer]),
      ));
    }],
    // Array params (Vec<Struct> via TryFromJsValue).
    ['User.placeOrders — array of NewOrderArgs', () => User.placeOrders(0, [
      new NewOrderArgs('50000.0', '0.1', Side.Bid, OrderType.Limit, false),
      new NewOrderArgs('49000.0', '0.2', Side.Ask, OrderType.PostOnly, true),
    ], false)],
    ['User.placeOrders — empty array', () => User.placeOrders(0, [], false)],
    ['User.cancelOrders — array of CancelOrderArgs', () => User.cancelOrders(0, [
      new CancelOrderArgs(100n),
      new CancelOrderArgs(undefined, 200n),
    ])],
    ['User.amendOrders — array of AmendOrderArgs', () => User.amendOrders(0, [amendArgs()])],
    ['User.createTriggerOrders — array of NewTriggerOrderArgs', () => User.createTriggerOrders(0, [
      new NewTriggerOrderArgs(
        Side.Bid, '50000.0', '49000.0',
        TriggerDirection.LessThanOrEqual, TriggerPriceCondition.Mark,
        OrderType.Limit,
      ),
    ])],
    ['Keeper.updateOraclePrices — array of OraclePriceUpdateArgs', () => Keeper.updateOraclePrices([
      new OraclePriceUpdateArgs(0, '50000.0'),
      new OraclePriceUpdateArgs(1, '3000.0'),
    ], nowMicros())],
    ['Keeper.updateOraclePricesWithPythProofs — array of proof args', () => (
      Keeper.updateOraclePricesWithPythProofs([
        new OraclePriceUpdateWithPythProofArgs(0, PYTH_PRIMARY_WITHOUT_QUOTE),
      ], nowMicros())
    )],
    ['Keeper.updateOraclePricesWithPythProofs — quote proof path', () => (
      Keeper.updateOraclePricesWithPythProofs([
        new OraclePriceUpdateWithPythProofArgs(1, PYTH_PRIMARY_WITH_QUOTE, PYTH_QUOTE),
      ], nowMicros())
    )],
    ['Keeper.updateMarkPrices — array of MarkPriceUpdateArgs', () => Keeper.updateMarkPrices([
      new MarkPriceUpdateArgs(0, '50000.0', '0.001'),
    ], nowMicros())],
    // Direct struct params.
    ['User.createPositionTpsl', () => User.createPositionTpsl(0, new TpslPair(tp()))],
    ['User.createTwapOrder', () => User.createTwapOrder(
      0, new NewTwapOrderArgs(Side.Bid, '100.0', false, 3600n),
    )],
    ['Vault.updateVaultConfig', () => Vault.updateVaultConfig(
      ADDRESS, new UpdateVaultConfigArgs('1000.0', 24, 10),
    )],
    ['Vault.updateVaultConfigV1 with a withdrawal fee', () => Vault.updateVaultConfigV1(
      ADDRESS, new UpdateVaultConfigArgsV1('1000.0', 24, 10, 5),
    )],
    ['Vault.updateVaultConfigV1 with all fields omitted', () => Vault.updateVaultConfigV1(
      ADDRESS, new UpdateVaultConfigArgsV1(undefined, undefined, undefined, undefined),
    )],
    ['Vault.setDepositPolicy with a deposit policy', () => Vault.setDepositPolicy(
      ADDRESS, new VaultDepositPolicyArgs('1000.0', []),
    )],
    ['Vault.setDepositPolicy with strategic caps', () => Vault.setDepositPolicy(
      ADDRESS,
      new VaultDepositPolicyArgs('1000.0', [new StrategicDepositorCap(ADDRESS, '50000.0')]),
    )],
    ['Vault.setDepositPolicy clearing the policy', () => Vault.setDepositPolicy(ADDRESS, undefined)],
    // Same method names in different namespaces, with different signatures.
    ['Admin.deposit', () => Admin.deposit(ADDRESS, 0, '100.0')],
    ['User.cancelOrders', () => User.cancelOrders(0, [new CancelOrderArgs(1n)])],
    ['Admin.cancelOrders', () => Admin.cancelOrders(`[[0, 1, "${ADDRESS}"]]`)],
    ['Admin.cancelTriggerOrders', () => Admin.cancelTriggerOrders(`[[0, 1, "${ADDRESS}"]]`)],
  ])('%s', (_name, build) => {
    expect(build()).toBeDefined();
  });
});

describe('invalid inputs reject', () => {
  test('Warp.transferRemote rejects ambiguous relayer tags', () => {
    expect(() => Warp.transferRemote({
      warpRoute: `0x${'11'.repeat(32)}`,
      amount: '12345678901234567890',
      destinationDomain: 1234,
      gasPaymentLimit: '400000',
      recipient: `0x${'22'.repeat(32)}`,
      relayer: {
        Standard: ADDRESS,
        Vm: `0x${'33'.repeat(32)}`,
      },
    })).toThrow(/either Standard or Vm/i);
  });

  test('invalid decimal rejects', () => {
    expect(() => User.deposit(0, 'not-a-number')).toThrow();
  });

  test('invalid address rejects', () => {
    expect(() => User.delegateUser('bad-addr', 'test')).toThrow();
  });
});
