/**
 * Provider authentication-failure hints.
 *
 * Claude Code surfaces provider auth errors (401/403 from Anthropic-compatible
 * gateways) as opaque text inside a session error. These markers are stable
 * enough across providers to detect; when matched we append a hint pointing at
 * the model profile settings (API 地址 / 模型 ID / 密钥权限) since that is the
 * usual cause in CC Panel.
 */

const AUTH_FAILURE_PATTERN =
  /failed to authenticate|api error: 40[13]|authentication error|unauthorized|no access to model/i;

export const AUTH_FAILURE_HINT =
  "API 鉴权失败：请检查模型配置的 API 地址、模型 ID，并确认该密钥对此模型有访问权限。";

/** Returns the hint text when `message` looks like a provider auth failure. */
export function authFailureHint(message: string): string | null {
  return AUTH_FAILURE_PATTERN.test(message) ? AUTH_FAILURE_HINT : null;
}
