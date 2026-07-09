import { readFileSync } from "node:fs";

import { jest } from "@jest/globals";

import {
  User as CallUser,
  Warp as CallWarp,
  isRuntimeCallData,
  toRuntimeCallJson,
} from "../pkg/calls";
import {
  BulletSdkError as StartupBulletSdkError,
  asBulletSdkError,
  isBulletSdkError,
} from "../pkg/errors";
import {
  KlineInterval as StartupKlineInterval,
  OrderbookDepth as StartupOrderbookDepth,
  Topic as StartupTopic,
} from "../pkg/topics";
import {
  NewOrderArgs as StartupNewOrderArgs,
  OrderType as StartupOrderType,
  Side as StartupSide,
} from "../pkg/primitives";
import { Client, RuntimeCall, Transaction } from "../pkg/node";

jest.setTimeout(30_000);

describe("startup-safe subpaths", () => {
  test("are exposed as dedicated package subpaths", () => {
    const packageJson = JSON.parse(
      readFileSync(new URL("../package.json", import.meta.url), "utf8"),
    );

    expect(packageJson.exports["./calls"]).toEqual({
      types: "./pkg/calls.d.ts",
      default: "./pkg/calls.js",
    });
    expect(packageJson.exports["./topics"]).toEqual({
      types: "./pkg/topics.d.ts",
      default: "./pkg/topics.js",
    });
    expect(packageJson.exports["./errors"]).toEqual({
      types: "./pkg/errors.d.ts",
      default: "./pkg/errors.js",
    });
    expect(packageJson.exports["./primitives"]).toEqual({
      types: "./pkg/primitives.d.ts",
      default: "./pkg/primitives.js",
    });
  });

  test("do not import the generated wasm-bindgen glue", () => {
    const subpaths = ["calls", "topics", "errors", "primitives"];

    for (const subpath of subpaths) {
      const source = readFileSync(new URL(`../pkg/${subpath}.js`, import.meta.url), "utf8");

      expect(source).not.toContain("bullet_rust_sdk_wasm");
      expect(source).not.toContain("_bg.wasm");
      expect(source).not.toContain("initSync");
    }
  });

  test("share internal startup helpers without exporting them as public subpaths", () => {
    const packageJson = JSON.parse(
      readFileSync(new URL("../package.json", import.meta.url), "utf8"),
    );
    const callsSource = readFileSync(new URL("../pkg/calls.js", import.meta.url), "utf8");
    const primitivesSource = readFileSync(
      new URL("../pkg/primitives.js", import.meta.url),
      "utf8",
    );
    const sharedSource = readFileSync(
      new URL("../pkg/startup-shared.js", import.meta.url),
      "utf8",
    );

    expect(packageJson.exports["./startup-shared"]).toBeUndefined();
    expect(callsSource).toContain('from "./startup-shared.js"');
    expect(primitivesSource).toContain('from "./startup-shared.js"');
    expect(callsSource).not.toContain("function normalizeValue");
    expect(primitivesSource).not.toContain("function normalizeValue");
    expect(sharedSource).not.toContain("bullet_rust_sdk_wasm");
    expect(sharedSource).not.toContain("_bg.wasm");
    expect(sharedSource).not.toContain("initSync");
  });

  test("builds canonical runtime-call data", () => {
    expect(CallUser.cancelAllOrders()).toEqual({
      exchange: {
        user: {
          cancel_all_orders: {},
        },
      },
    });

    expect(isRuntimeCallData(CallUser.cancelAllOrders())).toBe(true);
    expect(toRuntimeCallJson(CallUser.cancelAllOrders())).toBe(
      JSON.stringify({
        exchange: {
          user: {
            cancel_all_orders: {},
          },
        },
      }),
    );
  });

  test("uses serde enum values and plain struct data", () => {
    expect(StartupSide.Bid).toBe("bid");
    expect(StartupOrderType.Limit).toBe("limit");

    const order = new StartupNewOrderArgs(
      "50000.0",
      "0.1",
      StartupSide.Bid,
      StartupOrderType.Limit,
      false,
    );

    expect(order.toJSON()).toEqual({
      price: "50000.0",
      size: "0.1",
      side: "bid",
      order_type: "limit",
      reduce_only: false,
    });

    expect(CallUser.placeOrders(0, [order], false)).toEqual({
      exchange: {
        user: {
          place_orders: {
            market_id: 0,
            orders: [order.toJSON()],
            replace: false,
          },
        },
      },
    });
    expect(RuntimeCall.fromCall(CallUser.placeOrders(0, [order], false))).toBeDefined();
  });

  test("parses calls through the full wasm runtime boundary", () => {
    const call = CallUser.cancelAllOrders();

    expect(RuntimeCall.fromCall(call)).toBeDefined();
    expect(Transaction.builder().call(RuntimeCall.fromCall(call))).toBeDefined();
    expect(typeof Client.prototype.sendCall).toBe("function");
  });

  test("builds warp calls and rejects unsafe numeric amounts", () => {
    const warpRoute = `0x${"11".repeat(32)}`;
    const recipient = `0x${"22".repeat(32)}`;
    const relayer = "11111111111111111111111111111111";

    const warpCall = CallWarp.transferRemote({
      warpRoute,
      amount: "12345678901234567890",
      destinationDomain: 1234,
      gasPaymentLimit: "400000",
      recipient,
      relayer: { Standard: relayer },
    });

    expect(warpCall).toEqual({
      warp: {
        transfer_remote: {
          warp_route: warpRoute,
          destination_domain: 1234,
          recipient,
          amount: "12345678901234567890",
          relayer,
          gas_payment_limit: "400000",
        },
      },
    });
    expect(RuntimeCall.fromCall(warpCall)).toBeDefined();

    expect(() => CallWarp.transferRemote({
      warpRoute,
      amount: Number.MAX_SAFE_INTEGER + 1,
      destinationDomain: 1234,
      gasPaymentLimit: "400000",
      recipient,
      relayer: null,
    })).toThrow(/safe integer/);
  });

  test("builds websocket topic strings without wasm", () => {
    expect(StartupTopic.aggTrade("BTC-USD").toString()).toBe("BTC-USD@aggTrade");
    expect(StartupTopic.depth("BTC-USD", StartupOrderbookDepth.D10).toString()).toBe(
      "BTC-USD@depth10",
    );
    expect(StartupTopic.kline("BTC-USD", StartupKlineInterval.H1).toString()).toBe(
      "BTC-USD@kline_1h",
    );
    expect(StartupTopic.allBookTickers().toString()).toBe("!bookTicker@arr");
  });

  test("classifies BulletSdkError by shape", () => {
    const instance = new StartupBulletSdkError("rate limited", {
      kind: "api",
      status: 429,
      retryable: true,
    });
    const shaped = {
      name: "BulletSdkError",
      message: "validation failed",
      kind: "validation",
      retryable: false,
    };

    expect(isBulletSdkError(instance)).toBe(true);
    expect(isBulletSdkError(shaped)).toBe(true);
    expect(asBulletSdkError(shaped)?.kind).toBe("validation");
    expect(isBulletSdkError(new Error("nope"))).toBe(false);
  });
});
