# Bullet SDK — WASM Examples

Demonstrates using `@bulletxyz/sdk-wasm` across Node.js, Deno, and the browser.

## Prerequisites

Build the WASM packages first (from the repo root):

```bash
just build-wasm
```

## Structure

```
examples/
├── turbo.json          # Turborepo config (orchestrates node + web)
├── package.json        # Workspace root
├── node/               # Node.js (CJS via wasm-pack nodejs target)
│   ├── index.mjs       # Demo script
│   └── test.mjs        # node:test tests
├── deno/               # Deno (ESM via wasm-pack web target)
│   ├── index.ts        # Demo script
│   └── test.ts         # Deno.test tests
└── web/                # Next.js + Turbopack (browser via wasm-pack web target)
    ├── app/page.tsx    # React page loading WASM
    ├── test.spec.ts    # Playwright e2e test
    └── next.config.mjs # WASM webpack config
```

## Running

### All (Node + Web via Turbo)

```bash
cd examples
npm install
npm test          # runs node tests + web build
npm run dev       # starts Next.js dev server with Turbopack
```

### Node.js only

```bash
cd examples/node
npm install
npm start         # run demo
npm test          # run tests
```

### Deno only

```bash
cd examples/deno
deno task start   # run demo
deno task test    # run tests
```

### Web only

```bash
cd examples/web
npm install
npm run dev       # Next.js + Turbopack dev server at http://localhost:3000
npm run build     # production build
npm test          # Playwright e2e (starts dev server automatically)
```

## Environment

Set `BULLET_API_ENDPOINT` to use a different API endpoint (defaults to mainnet).
