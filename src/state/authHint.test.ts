import { describe, expect, it } from "vitest";
import { AUTH_FAILURE_HINT, authFailureHint } from "./authHint";

describe("authFailureHint", () => {
  it("detects the DeepSeek 403 authentication failure the user hit", () => {
    const message =
      "Failed to authenticate. API Error: 403 This token has no access to model ds (request id: 202608200404034982716298268d9d6VZnRYCrU)";
    expect(authFailureHint(message)).toBe(AUTH_FAILURE_HINT);
  });

  it("detects 401, 403, and unauthorized variants", () => {
    expect(authFailureHint("API Error: 401 invalid x-api-key")).toBe(
      AUTH_FAILURE_HINT,
    );
    expect(authFailureHint("Authentication error: invalid authorization")).toBe(
      AUTH_FAILURE_HINT,
    );
    expect(authFailureHint("API Error: 403 no access to model")).toBe(
      AUTH_FAILURE_HINT,
    );
  });

  it("returns null for unrelated errors", () => {
    expect(authFailureHint("模型拒绝了该操作。")).toBeNull();
    expect(authFailureHint("Rate limit exceeded, retry later")).toBeNull();
    expect(authFailureHint("")).toBeNull();
  });
});
