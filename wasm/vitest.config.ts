import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    // Keeps describe/it/expect ambient, as jest had them, so the suites need no edits.
    globals: true,
    include: ['tests/**/*.test.ts'],
    exclude: ['pkg/**', 'node_modules/**'],
    // Replaces the per-file jest.setTimeout(30_000) every suite used to call.
    testTimeout: 30_000,
  },
});
