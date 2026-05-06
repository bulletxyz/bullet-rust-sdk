"use client";

import { useEffect, useState } from "react";
import init, {
  Client,
  Keypair,
  User,
  NewOrderArgs,
  Side,
  OrderType,
  Transaction,
} from "@bulletxyz/sdk-wasm";

let wasmInitPromise: ReturnType<typeof init> | null = null;

function initWasm() {
  wasmInitPromise ??= init();
  return wasmInitPromise;
}

interface TxResult {
  publicKey: string;
  base64Length: number;
}

/**
 * Client Component — runs the WASM SDK in the browser.
 *
 * Demonstrates transaction building entirely client-side.
 * This is the pattern for operations that need a private key (signing),
 * which should never happen on the server.
 *
 * The web target requires an explicit init() call to load the WASM binary.
 */
export function ClientDemo() {
  const [txResult, setTxResult] = useState<TxResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        await initWasm();

        const client = await Client.builder()
          .network("https://tradingapi.bullet.xyz")
          .userActions(["PlaceOrders"])
          .build();

        // Build a demo limit order
        const order = new NewOrderArgs(
          "50000.0",
          "0.01",
          Side.Bid,
          OrderType.Limit,
          false,
        );
        const callMsg = User.placeOrders(0, [order], false);

        // Sign with a throwaway keypair (in a real app, use the user's key)
        const kp = Keypair.generate();
        const publicKey = kp.addressHex();
        const tx = Transaction.builder()
          .callMessage(callMsg)
          .maxFee(10_000_000n)
          .signer(kp)
          .build(client);

        if (cancelled) return;
        setTxResult({
          publicKey,
          base64Length: tx.toBase64().length,
        });
      } catch (err: any) {
        if (cancelled) return;
        setError(err.message ?? String(err));
      } finally {
        if (cancelled) return;
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <p>Initializing WASM in browser…</p>;
  if (error) return <p style={{ color: "red" }}>Error: {error}</p>;

  return (
    <div>
      <h3>Signed Transaction</h3>
      <p>
        Keypair: <code style={{ fontSize: "0.85em" }}>{txResult?.publicKey}</code>
      </p>
      <p>Base64 payload: {txResult?.base64Length} chars</p>
      <p style={{ color: "#666", fontSize: "0.85em" }}>
        This was built and signed entirely in the browser — the private key
        never left this tab.
      </p>
    </div>
  );
}
