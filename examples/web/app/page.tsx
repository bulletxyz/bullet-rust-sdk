import { Suspense } from "react";
import { ExchangeInfo } from "./server-data";
import { ClientDemo } from "./client-demo";

/**
 * Server Component — fetches exchange data at request time using the
 * WASM SDK on the Node.js runtime (SSR). No WASM init needed server-side
 * because the Node.js wasm-pack target loads synchronously.
 */
export default function Home() {
  return (
    <main>
      <section>
        <h2>Server-Side Rendered (SSR)</h2>
        <p style={{ color: "#666" }}>
          Exchange info fetched on the server via the Node.js WASM target.
        </p>
        <Suspense fallback={<p>Loading exchange info…</p>}>
          <ExchangeInfo />
        </Suspense>
      </section>

      <hr style={{ margin: "2rem 0" }} />

      <section>
        <h2>Client-Side Rendered (CSR)</h2>
        <p style={{ color: "#666" }}>
          Transaction building runs in the browser via the web WASM target.
        </p>
        <ClientDemo />
      </section>
    </main>
  );
}
