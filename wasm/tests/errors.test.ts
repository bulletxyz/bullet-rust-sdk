import { BulletSdkError, Decimal } from "../pkg/node";

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
});
