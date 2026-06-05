/**
 * Create a vault on Bullet, then optionally whitelist depositors and delegate
 * trading to another key. Leader-only; everything routes through the trading API.
 *
 * The vault address is derived locally from its name (`deriveVaultAddress`) —
 * the trading API has no vault lookup and CreateVault emits no address event.
 *
 * Usage:
 *   NETWORK=testnet node create_vault.ts <leader-keypair.json> [delegate-address]
 *
 * <leader-keypair.json> is a JSON array of 32 or 64 bytes (Solana style). The
 * leader signs and must already hold USDC to cover the creation fee.
 *
 * Requires Node >= 23.6 (native TypeScript).
 */

import { readFileSync } from "node:fs";
import {
  Client,
  CreateVaultArgs,
  Keypair,
  User,
  Vault,
  deriveVaultAddress,
} from "@bulletxyz/sdk-wasm";

// Vault settings. `whitelist` and `withdrawalFeeBps` are fixed at creation —
// updateVaultConfig can't change them later.
const VAULT = {
  name: "My Vault", // also determines the vault address
  description: "",
  depositAssets: ["USDC"],
  withdrawAsset: "USDC",
  lockupHours: 0,
  whitelist: false,
  profitSharePct: 100, // leader's cut of profits, 0–100
  withdrawalFeeBps: 0,
  depositLimit: "1000000",
};

// Addresses to allow as depositors (only takes effect when `whitelist` is true).
const WHITELIST_DEPOSITORS: string[] = [];

const [keypairPath, delegate] = process.argv.slice(2);
if (!keypairPath) {
  throw new Error(
    "usage: NETWORK=<testnet|mainnet> node create_vault.ts <leader-keypair.json> [delegate-address]",
  );
}

const secret = new Uint8Array(JSON.parse(readFileSync(keypairPath, "utf8"))).slice(0, 32);
const leader = Keypair.fromBytes(secret);
const leaderAddress = leader.address(); // capture before the builder consumes the keypair

const client = await Client.builder()
  .network(process.env.NETWORK ?? "testnet")
  .keypair(leader)
  .build();

const { assets } = await client.exchangeInfo();
const assetId = (name: string): number => {
  const asset = assets.find((a) => a.asset === name);
  if (!asset) throw new Error(`unknown asset: ${name}`);
  return asset.assetId;
};

const vault = deriveVaultAddress(VAULT.name);
console.log(`leader: ${leaderAddress}`);
console.log(`vault:  ${vault}\n`);

// CreateVaultArgs takes positional args, in this order:
const args = new CreateVaultArgs(
  VAULT.name,
  VAULT.description,
  leaderAddress,
  Uint16Array.from(VAULT.depositAssets.map(assetId)),
  assetId(VAULT.withdrawAsset),
  VAULT.lockupHours,
  VAULT.whitelist,
  VAULT.profitSharePct,
  VAULT.withdrawalFeeBps,
  VAULT.depositLimit,
);

// sendCallMessage builds, signs, and submits the tx. `status` is the queue
// state ("submitted"); the actual execution result is in `receipt.result`.
const created = await client.sendCallMessage(User.createVault(args));
console.log(`create: ${created.receipt?.result ?? created.status} (${created.id})`);

for (const depositor of WHITELIST_DEPOSITORS) {
  const res = await client.sendCallMessage(Vault.whitelistDepositor(vault, depositor));
  console.log(`whitelist ${depositor}: ${res.receipt?.result ?? res.status} (${res.id})`);
}

if (delegate) {
  const res = await client.sendCallMessage(Vault.delegateVaultUserV1(vault, delegate, "delegate", 0));
  console.log(`delegate ${delegate}: ${res.receipt?.result ?? res.status} (${res.id})`);
}
