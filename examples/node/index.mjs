/**
 * Bullet SDK — Node.js example
 *
 * Demonstrates: connecting to the exchange, querying market data,
 * constructing orders, and building signed transactions.
 *
 * Usage:
 *   npm start
 *
 * Set BULLET_API_ENDPOINT to override the default mainnet endpoint.
 */

import { createRequire } from "module";
const require = createRequire(import.meta.url);

const {
  Client,
  Keypair,
  User,
  NewOrderArgs,
  Side,
  OrderType,
  Transaction,
} = require("@bulletxyz/sdk-wasm");

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? "https://tradingapi.bullet.xyz";

async function main() {
  // Connect to the exchange
  console.log(`Connecting to ${ENDPOINT}…`);
  const client = await Client.connect(ENDPOINT);
  console.log("Connected.\n");

  // Fetch exchange info
  const info = await client.exchangeInfo();
  console.log(`Markets: ${info.symbols.length}`);
  console.log(`Assets:  ${info.assets.length}\n`);

  for (const sym of info.symbols.slice(0, 5)) {
    console.log(` • ${sym.symbol} (id=${sym.marketId})`);
  }
  if (info.symbols.length > 5) {
    console.log(`   … and ${info.symbols.length - 5} more\n`);
  }

  // Fetch a ticker
  const firstSymbol = info.symbols[0].symbol;
  const [ticker] = await client.tickerPrice(firstSymbol);
  console.log(`${firstSymbol} price: ${ticker.price}\n`);

  // Build a signed transaction
  const order = new NewOrderArgs(
    "50000.0",
    "0.01",
    Side.Bid,
    OrderType.Limit,
    false,
  );
  const callMsg = User.placeOrders(0, [order], false);
  console.log("CallMessage created:", callMsg.constructor.name);

  // Sign with a throwaway keypair
  const kp = Keypair.generate();
  console.log(`Generated keypair: ${kp.publicKeyHex()}`);

  const tx = Transaction.builder()
    .callMessage(callMsg)
    .maxFee(10_000_000n)
    .signer(kp)
    .build(client);
  const b64 = tx.toBase64();
  console.log("Signed transaction built successfully.");
  console.log(`  base64 length: ${b64.length} chars`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
