export class BulletSdkError extends Error {
    constructor(message, options = {}) {
        super(message);
        this.name = "BulletSdkError";
        this.kind = options.kind ?? "unknown";
        this.status = options.status;
        this.details = options.details;
        this.errorId = options.errorId;
        this.retryable = options.retryable ?? false;
    }
}
