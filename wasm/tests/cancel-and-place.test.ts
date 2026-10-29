import { describe, expect, test } from "vitest";
import { connectForUserActions } from "./helpers";

// Argument validation runs before anything is signed or submitted, so these never send a tx.
describe("cancelAndPlaceOrders argument validation", () => {
  test("is exported and rejects places that are not NewOrderArgs", async () => {
    const client = await connectForUserActions(["CancelAndPlaceOrders"]);
    expect(typeof client.cancelAndPlaceOrders).toBe("function");
    await expect(client.cancelAndPlaceOrders(0, "[]", [{ price: "1" }])).rejects.toThrow();
  });

  test("rejects malformed cancels JSON", async () => {
    const client = await connectForUserActions(["CancelAndPlaceOrders"]);
    await expect(client.cancelAndPlaceOrders(0, "not json", [])).rejects.toThrow();
  });
});
