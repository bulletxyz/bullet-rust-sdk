import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "test.spec.ts",
  webServer: {
    command: "npm run dev",
    port: 3000,
    reuseExistingServer: true,
    timeout: 30_000,
  },
  use: {
    baseURL: "http://localhost:3000",
  },
});
