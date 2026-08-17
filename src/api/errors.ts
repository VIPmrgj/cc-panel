import type { ApiError } from "./dto";

export class CcPanelError extends Error {
  readonly code: string;
  readonly retryable: boolean;
  readonly field?: string | null;
  readonly details?: ApiError["details"];

  constructor(error: ApiError) {
    super(error.message);
    this.name = "CcPanelError";
    this.code = error.code;
    this.retryable = error.retryable;
    this.field = error.field;
    this.details = error.details;
  }
}

function isApiError(value: unknown): value is ApiError {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.code === "string" &&
    typeof record.message === "string" &&
    typeof record.retryable === "boolean"
  );
}

export function normalizeInvokeError(value: unknown): CcPanelError {
  if (isApiError(value)) return new CcPanelError(value);
  if (value instanceof Error) {
    return new CcPanelError({
      code: "UNEXPECTED_CLIENT_ERROR",
      message: value.message || "发生未知错误。",
      retryable: false,
    });
  }
  return new CcPanelError({
    code: "UNEXPECTED_BACKEND_ERROR",
    message: typeof value === "string" ? value : "后端返回了无法识别的错误。",
    retryable: false,
  });
}
