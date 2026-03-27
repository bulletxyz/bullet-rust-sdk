"use client";

import dynamic from "next/dynamic";

// next/dynamic with ssr:false must be in a Client Component
const ClientDemo = dynamic(
  () => import("./client-demo").then((m) => ({ default: m.ClientDemo })),
  {
    ssr: false,
    loading: () => <p>Initializing WASM in browser…</p>,
  },
);

export function ClientDemoLoader() {
  return <ClientDemo />;
}
