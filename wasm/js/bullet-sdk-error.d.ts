export type BulletSdkErrorKind =
    | "api"
    | "http"
    | "websocket"
    | "validation"
    | "serialization"
    | "network"
    | "unknown";

export interface BulletSdkErrorOptions {
    kind?: BulletSdkErrorKind;
    status?: number;
    details?: unknown;
    retryable?: boolean;
}

export class BulletSdkError extends Error {
    readonly kind: BulletSdkErrorKind;
    readonly status?: number;
    readonly details?: unknown;
    readonly retryable: boolean;

    constructor(message: string, options?: BulletSdkErrorOptions);
}
