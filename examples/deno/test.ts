/**
 * Bullet SDK — Deno tests
 *
 * Run with: deno task test
 */

import { assertEquals } from "jsr:@std/assert";
import { assertThrows } from "jsr:@std/assert/throws";
import init, {
  Client,
  Keypair,
  User,
  NewOrderArgs,
  Side,
  OrderType,
  Transaction,
} from "@bulletxyz/sdk-wasm";

await init();

const ENDPOINT =
  Deno.env.get("BULLET_API_ENDPOINT") ?? "https://tradingapi.bullet.xyz";

Deno.test("REST API — connects and fetches exchange info", async () => {
  const client = await Client.connect(ENDPOINT);
  const info = await client.exchangeInfo();

  assertEquals(Array.isArray(info.symbols), true);
  assertEquals(info.symbols.length > 0, true);
  assertEquals(Array.isArray(info.assets), true);
  assertEquals(info.assets.length > 0, true);
});

Deno.test("REST API — fetches ticker price", async () => {
  const client = await Client.connect(ENDPOINT);
  const info = await client.exchangeInfo();
  const symbol = info.symbols[0].symbol;
  const [ticker] = await client.tickerPrice(symbol);

  assertEquals(typeof ticker.symbol, "string");
  assertEquals(!!ticker.price, true);
});

Deno.test("Transaction — constructs and signs via builder", async () => {
  const client = await Client.connect(ENDPOINT);

  const order = new NewOrderArgs(
    "50000.0",
    "0.01",
    Side.Bid,
    OrderType.Limit,
    false,
  );
  const callMsg = User.placeOrders(0, [order], false);

  const kp = Keypair.generate();
  const tx = Transaction.builder()
    .callMessage(callMsg)
    .maxFee(10_000_000n)
    .signer(kp)
    .build(client);

  const b64 = tx.toBase64();
  assertEquals(b64.length > 0, true);
});

Deno.test("Keypair — hex round-trip", () => {
  const kp = Keypair.generate();
  const hex = kp.publicKeyHex();
  assertEquals(typeof hex, "string");
  assertEquals(hex.length, 64);
});

Deno.test("Validation — rejects invalid decimals", () => {
  assertThrows(() => User.deposit(0, "not-a-number"));
});
