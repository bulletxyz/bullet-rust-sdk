/**
 * Server Component — uses the WASM SDK (Node.js target) during SSR.
 *
 * This demonstrates that the SDK works seamlessly on the server:
 * market data is fetched at request time and streamed to the client
 * as pre-rendered HTML.
 */

// Force dynamic rendering so we always get fresh data
export const dynamic = "force-dynamic";

const ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? "https://tradingapi.bullet.xyz";

export async function ExchangeInfo() {
  // On the server, Next.js resolves the "node" export condition
  // which gives us the synchronous CJS wasm-pack build.
  const { createRequire } = await import("module");
  const require = createRequire(import.meta.url);
  const sdk = require("@bulletxyz/sdk-wasm");

  const client = await sdk.Client.connect(ENDPOINT);
  const info = await client.exchangeInfo();

  const symbols: { symbol: string; marketId: number }[] = info.symbols.map(
    (s: any) => ({
      symbol: s.symbol,
      marketId: s.marketId,
    }),
  );

  const assets: { asset: string; assetId: number }[] = info.assets.map(
    (a: any) => ({
      asset: a.asset,
      assetId: a.assetId,
    }),
  );

  return (
    <div>
      <p>
        <strong>{symbols.length}</strong> markets,{" "}
        <strong>{assets.length}</strong> assets
      </p>

      <h3>Markets</h3>
      <table style={{ borderCollapse: "collapse", width: "100%" }}>
        <thead>
          <tr>
            <th style={{ textAlign: "left", padding: "4px 12px" }}>Symbol</th>
            <th style={{ textAlign: "left", padding: "4px 12px" }}>
              Market ID
            </th>
          </tr>
        </thead>
        <tbody>
          {symbols.map((s) => (
            <tr key={s.marketId}>
              <td style={{ padding: "4px 12px" }}>{s.symbol}</td>
              <td style={{ padding: "4px 12px" }}>{s.marketId}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>Assets</h3>
      <ul>
        {assets.map((a) => (
          <li key={a.assetId}>
            {a.asset} (id={a.assetId})
          </li>
        ))}
      </ul>
    </div>
  );
}
