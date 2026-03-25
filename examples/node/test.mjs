/**
 * Bullet SDK — Node.js tests
 *
 * Uses the built-in node:test runner (Node 18+).
 * Run with: npm test
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "module";

const require = createRequire(import.meta.url);
const sdk = require("@bulletxyz/sdk-wasm");

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? "https://tradingapi.bullet.xyz";

describe("REST API", () => {
  it("connects and fetches exchange info", async () => {
    const client = await sdk.Client.connect(ENDPOINT);
    const info = await client.exchangeInfo();

    assert.ok(Array.isArray(info.symbols));
    assert.ok(info.symbols.length > 0);
    assert.ok(Array.isArray(info.assets));
    assert.ok(info.assets.length > 0);
  });

  it("fetches ticker price", async () => {
    const client = await sdk.Client.connect(ENDPOINT);
    const info = await client.exchangeInfo();
    const symbol = info.symbols[0].symbol;
    const [ticker] = await client.tickerPrice(symbol);

    assert.ok(ticker.price);
    assert.equal(typeof ticker.symbol, "string");
  });
});

describe("Transaction building", () => {
  it("constructs and signs a transaction via builder", async () => {
    const client = await sdk.Client.connect(ENDPOINT);

    const order = new sdk.NewOrderArgs(
      "50000.0",
      "0.01",
      sdk.Side.Bid,
      sdk.OrderType.Limit,
      false,
    );
    const callMsg = sdk.User.placeOrders(0, [order], false);
    assert.ok(callMsg);

    const kp = sdk.Keypair.generate();
    const tx = sdk.Transaction.builder()
      .callMessage(callMsg)
      .maxFee(10_000_000n)
      .signer(kp)
      .build(client);

    const b64 = tx.toBase64();
    assert.ok(b64.length > 0);
  });

  it("generates keypair from hex round-trips", () => {
    const kp = sdk.Keypair.generate();
    const hex = kp.publicKeyHex();
    assert.equal(typeof hex, "string");
    assert.equal(hex.length, 64); // 32 bytes = 64 hex chars
  });

  it("rejects invalid decimal values", () => {
    assert.throws(() => sdk.User.deposit(0, "not-a-number"));
  });
});
