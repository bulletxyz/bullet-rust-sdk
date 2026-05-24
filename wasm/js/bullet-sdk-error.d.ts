export type BulletSdkErrorKind =
    | "api"
    | "http"
    | "websocket"
    | "validation"
    | "serialization"
    | "network"
    | "unknown";

export type JsonValue =
    | string
    | number
    | boolean
    | null
    | JsonValue[]
    | { [key: string]: JsonValue };

export type BulletSdkErrorDetails = JsonValue;

export interface BulletSdkErrorDetailsByKind {
    api: JsonValue;
    http: {
        cause?: string;
    };
    websocket: {
        code?: number;
        reason?: string;
    };
    validation: {
        field?: string;
        reason?: string;
    };
    serialization: {
        reason?: string;
    };
    network: {
        url?: string;
        reason?: string;
    };
    unknown: JsonValue;
}

export type BulletSdkErrorStatus<K extends BulletSdkErrorKind> =
    K extends "api" ? number : undefined;

export interface BulletSdkErrorOptions<K extends BulletSdkErrorKind = BulletSdkErrorKind> {
    kind?: K;
    status?: BulletSdkErrorStatus<K>;
    details?: BulletSdkErrorDetailsByKind[K];
    /** Server-side correlation id for support (only set for `kind === "api"`). */
    errorId?: string;
    retryable?: boolean;
}

export class BulletSdkError<K extends BulletSdkErrorKind = BulletSdkErrorKind> extends Error {
    readonly kind: K;
    readonly status?: BulletSdkErrorStatus<K>;
    readonly details?: BulletSdkErrorDetailsByKind[K];
    /** Server-side correlation id for support (only set for `kind === "api"`). */
    readonly errorId?: string;
    readonly retryable: boolean;

    constructor(message: string, options?: BulletSdkErrorOptions<K>);
}
