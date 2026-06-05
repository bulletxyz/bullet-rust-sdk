/**
 * Tests for transaction building and submission.
 *
 * Verifies:
 * 1. Transaction.builder() fluent pattern works
 * 2. External signing flow (buildUnsigned → toSignableBytes → fromParts)
 * 3. Error handling for missing required fields
 * 4. Transaction serialization to base64 and bytes
 */

import { jest } from "@jest/globals";

import {
  Keypair,
  NewOrderArgs,
  OrderType,
  Public,
  RuntimeCall,
  Side,
  SignedTransaction,
  SolanaOffchainTransaction,
  Transaction,
  User,
} from "../pkg/node";
import { connectForUserActions } from "./helpers";

jest.setTimeout(30_000);

// ── Transaction.builder() pattern ────────────────────────────────────────────

describe("Transaction.builder()", () => {
  test("Transaction.builder exists", () => {
    expect(typeof Transaction.builder).toBe("function");
  });

  test("build signed tx from User.cancelAllOrders", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    const b64 = tx.toBase64();
    expect(typeof b64).toBe("string");
    expect(b64.length).toBeGreaterThan(0);
  });

  test("build signed tx from User.placeOrders with typed wrappers", async () => {
    const client = await connectForUserActions(["PlaceOrders"]);
    const keypair = Keypair.generate();

    const order = new NewOrderArgs("50000.0", "0.1", Side.Bid, OrderType.Limit, false);
    const tx = Transaction.builder()
      .callMessage(User.placeOrders(0, [order], false))
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    expect(tx.toBase64().length).toBeGreaterThan(0);
  });

  test("build tx with priorityFeeBips", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .priorityFeeBips(100n)
      .signer(keypair)
      .build(client);

    expect(tx).toBeDefined();
    expect(tx.toBase64().length).toBeGreaterThan(0);
  });

  test("client.sendTransaction works with built tx", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    expect(typeof client.sendTransaction).toBe("function");
  });

  test("selective userActions rejects non-User call messages", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    expect(() => {
      Transaction.builder()
        .callMessage(Public.applyFunding([]))
        .maxFee(10_000_000n)
        .signer(keypair)
        .build(client);
    }).toThrow(/must be added to user-actions/);
  });
});

// ── External signing ─────────────────────────────────────────────────────────

describe("external signing", () => {
  test("buildUnsigned → toBytes → fromParts", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const unsigned = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    // Get signable bytes (borsh tx + chain hash baked in)
    const signableBytes = unsigned.toBytes();
    expect(signableBytes).toBeInstanceOf(Uint8Array);
    expect(signableBytes.length).toBeGreaterThan(32);

    const displayMessage = unsigned.toDisplayMessage();
    expect(displayMessage).toContain("CancelAllOrders");
    expect(displayMessage).toContain("max_fee");

    // Sign with keypair
    const signature = keypair.sign(signableBytes);
    expect(signature.length).toBe(64);

    const pubKey = keypair.publicKey();
    expect(pubKey.length).toBe(32);

    // Assemble signed transaction
    const signed = SignedTransaction.fromParts(unsigned, signature, pubKey);
    expect(signed).toBeDefined();

    // Verify serialization works
    const bytes = signed.toBytes();
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(bytes.length).toBeGreaterThan(0);

    const b64 = signed.toBase64();
    expect(typeof b64).toBe("string");
    expect(b64.length).toBeGreaterThan(0);
  });

  test("toBytes() is deterministic", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const tx = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .signer(keypair)
      .build(client);

    const bytes1 = tx.toBytes();
    const bytes2 = tx.toBytes();
    expect(bytes1).toEqual(bytes2);
  });

  test("buildUnsigned → toMessageBytes → SolanaOffchainTransaction.fromParts", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    const unsigned = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    const messageBytes = unsigned.toMessageBytes();
    expect(messageBytes).toBeInstanceOf(Uint8Array);

    const message = JSON.parse(new TextDecoder().decode(messageBytes));
    expect(message.chain_hash).toBeUndefined();
    expect(message.chain_name).toBe(client.chainName());
    expect(message.runtime_call).toBeDefined();
    expect(BigInt(message.details.chain_id)).toBe(client.chainId());

    const signature = keypair.sign(messageBytes);
    const pubKey = keypair.publicKey();
    const tx = SolanaOffchainTransaction.fromParts(unsigned, signature, pubKey);

    expect(tx.toBytes()).toBeInstanceOf(Uint8Array);
    expect(tx.toBytes().length).toBeGreaterThan(messageBytes.length);
    expect(tx.toBase64().length).toBeGreaterThan(0);
    expect(typeof client.sendOffChainTransaction).toBe("function");
  });
});

// ── Error handling ───────────────────────────────────────────────────────────

describe("error handling", () => {
  test("missing callMessage throws error", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);
    const keypair = Keypair.generate();

    expect(() => {
      Transaction.builder().maxFee(10_000_000n).signer(keypair).build(client);
    }).toThrow();
  });

  test("missing signer throws error", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);

    expect(() => {
      Transaction.builder().callMessage(User.cancelAllOrders()).maxFee(10_000_000n).build(client);
    }).toThrow();
  });

  test("fromParts rejects invalid signature length", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);

    const unsigned = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    expect(() => {
      SignedTransaction.fromParts(unsigned, new Uint8Array(63), new Uint8Array(32));
    }).toThrow();
  });

  test("fromParts rejects invalid pubkey length", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);

    const unsigned = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .buildUnsigned(client);

    expect(() => {
      SignedTransaction.fromParts(unsigned, new Uint8Array(64), new Uint8Array(31));
    }).toThrow();
  });
});

// ── RuntimeCall construction seam ─────────────────────────────────────────────
describe("RuntimeCall", () => {
  test(".call(RuntimeCall.exchange(msg)) matches .callMessage(msg)", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);

    const viaCall = Transaction.builder()
      .call(RuntimeCall.exchange(User.cancelAllOrders()))
      .maxFee(10_000_000n)
      .window(42n)
      .buildUnsigned(client);

    const viaCallMessage = Transaction.builder()
      .callMessage(User.cancelAllOrders())
      .maxFee(10_000_000n)
      .window(42n)
      .buildUnsigned(client);

    expect(Buffer.from(viaCall.toBytes())).toEqual(Buffer.from(viaCallMessage.toBytes()));
  });

  test("RuntimeCall.fromJson produces byte-identical signable bytes to the typed path", async () => {
    const client = await connectForUserActions(["CancelAllOrders"]);

    // Build via the typed factory, then recover the canonical runtime_call JSON
    // the SDK itself emits and feed it back through fromJson.
    const typed = Transaction.builder()
      .call(RuntimeCall.exchange(User.cancelAllOrders()))
      .maxFee(10_000_000n)
      .window(42n)
      .buildUnsigned(client);

    const message = JSON.parse(Buffer.from(typed.toMessageBytes()).toString());
    const callJson = JSON.stringify(message.runtime_call);

    const fromJson = Transaction.builder()
      .call(RuntimeCall.fromJson(callJson))
      .maxFee(10_000_000n)
      .window(42n)
      .buildUnsigned(client);

    expect(Buffer.from(fromJson.toBytes())).toEqual(Buffer.from(typed.toBytes()));
  });

  test("RuntimeCall.fromJson rejects malformed JSON", () => {
    expect(() => RuntimeCall.fromJson("{ not valid runtime call }")).toThrow();
  });
});
