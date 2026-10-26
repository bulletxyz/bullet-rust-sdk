/**
 * Tests for the fast-bridge deposit memo encoder.
 *
 * The byte layout is the contract between this encoder and the fast-bridge
 * operator's decoder. The same layout is pinned on the Rust side; if the two ever
 * disagreed, the operator would read these bytes as malformed and quietly credit
 * the depositor instead of the person they named.
 *
 * Layout: version(1) | toAddress(32) | Option<u8> subAccountIndex | balance bucket |
 *         u16 marketId little-endian (Iso only)
 *
 * Rust pairs the market with the bucket inside `Iso`; only this wrapper splits
 * them into two arguments, so only here can a caller name a combination the wire
 * format has no way to express. Both such combinations are rejected, and the
 * tests below pin that in each direction.
 */

import { Keypair, encodeFastBridgeMemo, BalanceBucket } from "../pkg/node";

// A real address, so the base58 decode under test is exercised rather than stubbed.
const RECIPIENT = Keypair.fromBytes(new Uint8Array(32).fill(7)).address();
const RECIPIENT_BYTES = new Uint8Array(
  Keypair.fromBytes(new Uint8Array(32).fill(7)).publicKey(),
);

test('encodes version, address and a master-account bank destination', () => {
  const memo = encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Bank);

  expect(memo.length).toBe(1 + 32 + 1 + 1);
  expect(memo[0]).toBe(1); // the V1 discriminant is the version byte
  expect(Array.from(memo.slice(1, 33))).toEqual(Array.from(RECIPIENT_BYTES));
  expect(memo[33]).toBe(0); // Option::None
  expect(memo[34]).toBe(0); // BalanceBucket::Bank
});

test('encodes a sub-account index as Option::Some', () => {
  const memo = encodeFastBridgeMemo(RECIPIENT, 3, BalanceBucket.Cross);

  expect(memo.length).toBe(1 + 32 + 2 + 1);
  expect(memo[33]).toBe(1); // Option::Some
  expect(memo[34]).toBe(3); // the index
  expect(memo[35]).toBe(1); // BalanceBucket::Cross
});

// The wrapper maps each JS bucket to its Rust counterpart by hand, so a swapped arm
// would route a deposit to the wrong ledger while every other test still passed.
test('encodes Spot as its own bucket, distinct from Bank and Cross', () => {
  const memo = encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Spot);

  expect(memo.length).toBe(1 + 32 + 1 + 1);
  expect(memo[33]).toBe(0); // Option::None
  expect(memo[34]).toBe(2); // BalanceBucket::Spot
});

test('encodes an Iso market id as little-endian u16', () => {
  const memo = encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Iso, 7);

  expect(memo.length).toBe(1 + 32 + 1 + 3);
  expect(memo[33]).toBe(0); // Option::None
  expect(memo[34]).toBe(3); // BalanceBucket::Iso
  expect(memo[35]).toBe(7); // market id, low byte first
  expect(memo[36]).toBe(0);
});

test('an Iso destination without a market id is rejected rather than guessed', () => {
  // Defaulting the market would silently credit the wrong ledger.
  expect(() => encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Iso))
    .toThrow(/marketId/);
});

// The other half of the same hole. Dropping the market id here would encode a
// plain Cross memo, and the operator — which only ever sees the bytes — would
// credit cross margin exactly as if that is what was asked for. Nobody finds out:
// the caller gets a memo back, the deposit succeeds, and the funds are not
// recoverable from the balance they land in.
test.each([
  ['Bank', BalanceBucket.Bank],
  ['Cross', BalanceBucket.Cross],
  ['Spot', BalanceBucket.Spot],
])('rejects a market id given with the non-Iso bucket %s', (_name, bucket) => {
  expect(() => encodeFastBridgeMemo(RECIPIENT, undefined, bucket, 7))
    .toThrow(/marketId/);
});

// Market 0 is a real market, and a `marketId &&`-style guard would wave it
// through as "not given" — the one value where a truthiness check misreads the
// caller.
test('rejects market id 0 with a non-Iso bucket, like any other market', () => {
  expect(() => encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Cross, 0))
    .toThrow(/marketId/);
});

// null and undefined both mean "not given" at the wasm boundary, so neither may
// trip the new rule — a JS caller building arguments up reaches for either.
test('treats an explicit null market id as absent for a non-Iso bucket', () => {
  const memo = encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Cross, null);

  expect(memo.length).toBe(1 + 32 + 1 + 1);
  expect(memo[34]).toBe(1); // BalanceBucket::Cross, with no market trailer
});

test('rejects an address that is not a real address', () => {
  expect(() => encodeFastBridgeMemo('not-an-address', undefined, BalanceBucket.Bank))
    .toThrow();
});

test('rejects a sub-account index that cannot exist', () => {
  // The runtime tracks sub-accounts in a 32-bit mask, so only 0..=31 exist.
  expect(() => encodeFastBridgeMemo(RECIPIENT, 32, BalanceBucket.Bank)).toThrow();
});

/**
 * A JS number reaches wasm through glue that used to reshape it rather than
 * refuse it. With `subAccountIndex` declared `u32`, wasm-bindgen emitted
 * `(x) >>> 0`, so `parseInt("")` on an empty form field arrived as sub-account
 * 0 — not the master account the caller meant — and `3.9` and `2**32 + 3` both
 * arrived as 3. Every one of those silently credits a destination nobody asked
 * for, and bridged funds do not come back, so each must throw instead.
 *
 * This can only be tested from JS: by the time Rust is reached the value has
 * already been reshaped, so a Rust test sees a perfectly ordinary integer.
 */
test.each([
  ['NaN, as parseInt("") yields', NaN],
  ['a fraction', 3.9],
  ['a value past u32', 2 ** 32 + 3],
  ['infinity', Infinity],
])('rejects %s as a sub-account index', (_label, value) => {
  expect(() =>
    encodeFastBridgeMemo(RECIPIENT, value, BalanceBucket.Cross),
  ).toThrow(/whole number/);
});

test('rejects NaN as a market id rather than crediting market 0', () => {
  expect(() =>
    encodeFastBridgeMemo(RECIPIENT, undefined, BalanceBucket.Iso, NaN),
  ).toThrow(/whole number/);
});

/** The reshaping is gone, but the values that were always valid still are. */
test('still encodes the sub-account indices at the edges of the range', () => {
  expect(encodeFastBridgeMemo(RECIPIENT, 0, BalanceBucket.Cross)[34]).toBe(0);
  expect(encodeFastBridgeMemo(RECIPIENT, 31, BalanceBucket.Cross)[34]).toBe(31);
});
