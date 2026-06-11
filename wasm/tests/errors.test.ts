import { BulletSdkError, Decimal } from "../pkg/node";
import type { BulletSdkErrorDetails } from "../pkg/node";

describe('BulletSdkError', () => {
  test('wasm-thrown errors use the exported error class', () => {
    try {
      new Decimal('not_a_number');
      throw new Error('expected Decimal constructor to throw');
    } catch (err: unknown) {
      expect(err).toBeInstanceOf(Error);
      expect(err).toBeInstanceOf(BulletSdkError);
      expect(err).toMatchObject({
        name: 'BulletSdkError',
        kind: 'validation',
        retryable: false,
      });
    }
  });

  test('constructor preserves typed structured details', () => {
    const details = {
      code: 'insufficient_funds',
      required: '10.0',
      nested: {
        retryAfterMs: 250,
      },
    } satisfies BulletSdkErrorDetails;

    const err = new BulletSdkError('API error', {
      kind: 'api',
      status: 400,
      details,
      retryable: false,
    });

    expect(err.details).toEqual(details);
  });

  test('kind generic narrows details type', () => {
    const err = new BulletSdkError<'validation'>('Invalid order', {
      kind: 'validation',
      details: {
        field: 'price',
        reason: 'must be positive',
      },
      retryable: false,
    });

    const reason: string | undefined = err.details?.reason;
    expect(reason).toBe('must be positive');
  });

  test('constructor preserves errorId for support correlation', () => {
    // Regression: the JS constructor previously dropped options.errorId on the
    // floor even though the Rust side set it, so `err.errorId` was always
    // undefined despite README documenting it.
    const err = new BulletSdkError('API error', {
      kind: 'api',
      status: 400,
      errorId: '8b2e4d9f-7a1c-4f0e-9c5d-3e6a8b1c2d4f',
      retryable: false,
    });

    expect(err.errorId).toBe('8b2e4d9f-7a1c-4f0e-9c5d-3e6a8b1c2d4f');
  });

  test('errorId is undefined when not provided', () => {
    const err = new BulletSdkError('Network error', {
      kind: 'network',
      retryable: true,
    });

    expect(err.errorId).toBeUndefined();
  });
});
