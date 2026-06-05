/**
 * Create and set up a vault (leader-only operations).
 *
 * One-shot script that, as the vault leader, walks the full setup:
 *   1. CreateVault       — name, assets, commission, fees, deposit limit, etc.
 *   2. whitelistDepositor — allow specific addresses (only meaningful when the
 *                           vault was created with whitelistDeposits: true)
 *   3. delegateVaultUserV1 — authorize a key to trade on the vault (optional)
 *   4. updateVaultConfig  — change deposit limit / lockup / commission (optional)
 *
 * Everything goes through the trading API; the rollup is never contacted. The
 * vault address is derived locally with `deriveVaultAddress(name)` (the trading
 * API exposes no vault-address lookup, and CreateVault emits no address event).
 *
 * Note on what's fixed at creation: `withdrawalFeeBps` and the `whitelistDeposits`
 * toggle can only be set in CreateVault — updateVaultConfig cannot change them.
 *
 * Usage:
 *   NETWORK=testnet node create_vault.ts <leader-keypair.json> [delegate-pubkey]
 *
 * Arguments:
 *   <leader-keypair.json>  Path to the leader's keypair file: a JSON array of
 *                          32 (secret) or 64 (Solana-style) bytes. The leader
 *                          signs, and becomes the vault leader. Relative paths
 *                          resolve against the current working directory.
 *   [delegate-pubkey]      Optional base58 address to delegate vault trading to.
 *                          Omit to skip delegation.
 *
 * Requires Node >= 23.6 (native TypeScript). Edit the config block below to tune
 * vault settings, whitelist depositors, and post-create config changes.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  Client,
  CreateVaultArgs,
  Keypair,
  UpdateVaultConfigArgs,
  User,
  Vault,
  deriveVaultAddress,
} from "@bulletxyz/sdk-wasm";

// =========================================================================
// EDIT THESE BEFORE RUNNING
// =========================================================================

const VAULT_PARAMS = {
  name: "Pick a unique name", // also determines the vault address
  description: "Vault description",
  depositAssets: ["USDC"],
  withdrawAsset: "USDC",
  withdrawLockupPeriodHours: 0,
  whitelistDeposits: false,
  // Whole-number percent (0–100) of vault profit that goes to the leader.
  profitSharePercentage: 100,
  withdrawalFeeBps: 0,
  depositLimit: "1000000",
};

const DELEGATE_PARAMS = {
  name: "vault-delegate",
  flags: 0,
  expiresAt: null as bigint | null,
};

// Base58 addresses to allow as depositors. Only takes effect when the vault is
// created with `whitelistDeposits: true`. Leave empty to skip.
const WHITELIST_DEPOSITORS: string[] = [];

// Optional config change applied right after creation (demonstrates the leader's
// updateVaultConfig op). Set to null to skip. Only these three fields can be
// changed post-creation; undefined fields are left unchanged.
const CONFIG_UPDATE: {
  depositLimit?: string;
  withdrawLockupPeriodHours?: number;
  profitSharePercentage?: number;
} | null = null;

// =========================================================================

/** Load a Keypair from a JSON array of 32 (secret) or 64 (Solana-style) bytes. */
function loadKeypair(path: string): Keypair {
  const bytes = new Uint8Array(JSON.parse(readFileSync(path, "utf-8")));
  return Keypair.fromBytes(bytes.slice(0, 32));
}

async function main() {
  const network = process.env.NETWORK ?? "testnet";

  const [keypairArg, delegateArg] = process.argv.slice(2);
  if (!keypairArg) {
    throw new Error(
      "Usage: NETWORK=<testnet|mainnet> node create_vault.ts <leader-keypair.json> [delegate-pubkey]",
    );
  }

  const leader = loadKeypair(resolve(process.cwd(), keypairArg));
  // Capture the address before `.keypair(leader)` — passing the keypair to the
  // builder consumes it (wasm-bindgen move), freeing the JS handle.
  const leaderAddress = leader.address();
  console.log(`Network: ${network}`);
  console.log(`Leader:  ${leaderAddress}\n`);

  const client = await Client.builder().network(network).keypair(leader).build();

  // Resolve asset names → ids from the trading API.
  const info = await client.exchangeInfo();
  const assetId = (name: string): number => {
    const asset = info.assets.find((a) => a.asset === name);
    if (!asset) throw new Error(`Unknown asset: ${name}`);
    return asset.assetId;
  };
  const depositAssetIds = Uint16Array.from(VAULT_PARAMS.depositAssets.map(assetId));
  const withdrawAssetId = assetId(VAULT_PARAMS.withdrawAsset);

  // 1. Create the vault.
  console.log(`Creating vault "${VAULT_PARAMS.name}"...`);
  const args = new CreateVaultArgs(
    VAULT_PARAMS.name,
    VAULT_PARAMS.description,
    leaderAddress,
    depositAssetIds,
    withdrawAssetId,
    VAULT_PARAMS.withdrawLockupPeriodHours,
    VAULT_PARAMS.whitelistDeposits,
    VAULT_PARAMS.profitSharePercentage,
    VAULT_PARAMS.withdrawalFeeBps,
    VAULT_PARAMS.depositLimit,
  );
  const createResp = await client.sendCallMessage(User.createVault(args));
  console.log(`  status: ${createResp.status} (tx ${createResp.id})\n`);

  const vaultAddress = deriveVaultAddress(VAULT_PARAMS.name);
  console.log(`Vault address: ${vaultAddress}\n`);

  // 2. Whitelist depositors (only meaningful when whitelistDeposits is true).
  if (WHITELIST_DEPOSITORS.length > 0) {
    if (!VAULT_PARAMS.whitelistDeposits) {
      console.warn("  Note: whitelistDeposits is false — these entries have no effect.\n");
    }
    for (const depositor of WHITELIST_DEPOSITORS) {
      console.log(`Whitelisting depositor ${depositor}...`);
      const resp = await client.sendCallMessage(
        Vault.whitelistDepositor(vaultAddress, depositor),
      );
      console.log(`  status: ${resp.status} (tx ${resp.id})`);
    }
    console.log();
  }

  // 3. Optionally delegate trading to another key.
  if (delegateArg) {
    console.log(`Delegating vault trading to ${delegateArg}...`);
    const delegateResp = await client.sendCallMessage(
      Vault.delegateVaultUserV1(
        vaultAddress,
        delegateArg,
        DELEGATE_PARAMS.name,
        DELEGATE_PARAMS.flags,
        DELEGATE_PARAMS.expiresAt,
      ),
    );
    console.log(`  status: ${delegateResp.status} (tx ${delegateResp.id})\n`);
  }

  // 4. Optionally apply a config change.
  if (CONFIG_UPDATE) {
    console.log("Updating vault config...");
    const updateArgs = new UpdateVaultConfigArgs(
      CONFIG_UPDATE.depositLimit,
      CONFIG_UPDATE.withdrawLockupPeriodHours,
      CONFIG_UPDATE.profitSharePercentage,
    );
    const resp = await client.sendCallMessage(
      Vault.updateVaultConfig(vaultAddress, updateArgs),
    );
    console.log(`  status: ${resp.status} (tx ${resp.id})\n`);
  }

  console.log("=".repeat(60));
  console.log("Done.");
  console.log(`  Network:      ${network}`);
  console.log(`  Leader:       ${leaderAddress}`);
  console.log(`  Vault:        ${vaultAddress}`);
  console.log(`  Profit share: ${VAULT_PARAMS.profitSharePercentage}%`);
  if (VAULT_PARAMS.whitelistDeposits) {
    console.log(`  Whitelisted:  ${WHITELIST_DEPOSITORS.length} depositor(s)`);
  }
  if (delegateArg) console.log(`  Delegate:     ${delegateArg}`);
  console.log("=".repeat(60));
}

main().catch((err) => {
  console.error("\nError:", err);
  process.exit(1);
});
