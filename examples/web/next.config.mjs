/** @type {import('next').NextConfig} */
const nextConfig = {
  // Enable WASM support
  webpack(config) {
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
    };
    return config;
  },
};

export default nextConfig;
