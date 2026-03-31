import { test, expect } from "@playwright/test";

test("SSR — exchange info is pre-rendered", async ({ page }) => {
  await page.goto("/");

  // The SSR section should render server-side, visible quickly
  await expect(page.getByText("Server-Side Rendered (SSR)")).toBeVisible();

  // Market table should be populated from the server
  await expect(page.locator("table tbody tr").first()).toBeVisible({
    timeout: 15_000,
  });

  // Should show market and asset counts
  await expect(page.getByText("markets")).toBeVisible();
  await expect(page.getByText("assets")).toBeVisible();
});

test("CSR — transaction is built client-side", async ({ page }) => {
  await page.goto("/");

  // Wait for client-side WASM to initialize and build a transaction
  await expect(page.getByText("Signed Transaction")).toBeVisible({
    timeout: 30_000,
  });

  // Keypair should be displayed as hex
  await expect(page.locator("code")).toBeVisible();

  // Base64 payload length should be shown
  await expect(page.getByText("chars")).toBeVisible();
});
