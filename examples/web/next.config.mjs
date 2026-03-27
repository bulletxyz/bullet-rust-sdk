/** @type {import('next').NextConfig} */
const nextConfig = {
  // Let Node.js resolve the WASM SDK natively on the server
  // (needed for any package that uses readFileSync or .wasm files)
  serverExternalPackages: ["@bulletxyz/sdk-wasm"],
};

export default nextConfig;
