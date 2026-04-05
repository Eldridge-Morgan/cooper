/**
 * Structured error codes with automatic HTTP status mapping.
 */
export type ErrorCode =
  | "NOT_FOUND"
  | "UNAUTHORIZED"
  | "PERMISSION_DENIED"
  | "RATE_LIMITED"
  | "INVALID_ARGUMENT"
  | "VALIDATION_FAILED"
  | "INTERNAL";

const STATUS_MAP: Record<ErrorCode, number> = {
  NOT_FOUND: 404,
  UNAUTHORIZED: 401,
  PERMISSION_DENIED: 403,
  RATE_LIMITED: 429,
  INVALID_ARGUMENT: 400,
  VALIDATION_FAILED: 422,
  INTERNAL: 500,
};

export class CooperError extends Error {
  public readonly code: ErrorCode;
  public readonly statusCode: number;

  constructor(code: ErrorCode, message: string) {
    super(message);
    this.name = "CooperError";
    this.code = code;
    this.statusCode = STATUS_MAP[code] ?? 500;
  }

  toJSON() {
    return {
      error: {
        code: this.code,
        message: this.message,
      },
    };
  }
}
