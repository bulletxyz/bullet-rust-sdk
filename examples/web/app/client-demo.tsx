"use client";

import { useEffect, useState } from "react";

interface TxResult {
  publicKey: string;
  base64Length: number;
}

/**
 * Client Component — loads the WASM SDK in the browser (web target)
 * and demonstrates transaction building entirely client-side.
 *
 * This is the pattern for operations that need a private key (signing),
 * which should never happen on the server.
 */
export function ClientDemo() {
  const [txResult, setTxResult] = useState<TxResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        // Dynamic import — loads the web WASM target in the browser
        const sdk = await import("@bulletxyz/sdk-wasm");
        await (sdk as any).default(); // init WASM

        const client = await sdk.Client.connect(
          "https://tradingapi.bullet.xyz",
        );

        // Build a demo limit order
        const order = new sdk.NewOrderArgs(
          "50000.0",
          "0.01",
          sdk.Side.Bid,
          sdk.OrderType.Limit,
          false,
        );
        const callMsg = sdk.User.placeOrders(0, [order], false);

        // Sign with a throwaway keypair (in a real app, use the user's key)
        const kp = sdk.Keypair.generate();
        const tx = sdk.Transaction.builder()
          .callMessage(callMsg)
          .maxFee(10_000_000n)
          .signer(kp)
          .build(client);

        setTxResult({
          publicKey: kp.publicKeyHex(),
          base64Length: tx.toBase64().length,
        });
      } catch (err: any) {
        setError(err.message ?? String(err));
      } finally {
        setLoading(false);
      }
    })();
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
