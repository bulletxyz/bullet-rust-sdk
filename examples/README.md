# WASM examples

Examples for `@bulletxyz/sdk-wasm` in Node.js (ESM), Deno and a Next.js browser app.
Build the package first with `just build-wasm` from the SDK root.

## Node.js and browser

```sh
cd examples
npm install --install-links
npm test
npm run dev
```

To run a single example, use its directory:

```sh
cd examples/node
npm install --install-links
npm start
NETWORK=testnet npm run create-vault -- <leader-keypair.json> [delegate-address]
```

The vault script requires Node.js 23.6 or later. In `examples/web`, use `npm run dev`,
`npm run build` or `npm test` (Playwright starts the development server).

## Deno

```sh
cd examples/deno
deno task start
deno task test
```

Set `BULLET_API_ENDPOINT` to override the endpoint. Demos default to mainnet;
Node.js and Deno tests default to testnet.
