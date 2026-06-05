/**
 * Tests for vault helpers.
 */

import { deriveVaultAddress } from "../pkg/node";

describe("deriveVaultAddress", () => {
  test("derives the vault address from its name (golden vector)", () => {
    // sha256("default") base58-encoded — matches the Rust golden and the
    // runtime's generate_address_with_seed(b"default").
    expect(deriveVaultAddress("default")).toBe(
      "4kGq3HJ6gYLf5ekoFgZJ3hGAUuHP6sK1v2LPs5zKHaCn",
    );
  });

  test("is name-sensitive", () => {
    expect(deriveVaultAddress("default")).not.toBe(deriveVaultAddress("Default"));
  });
});
