import type { ChatMessage } from "../api/dto";

export type PermissionRiskLevel = "low" | "high";

export interface PermissionRisk {
  level: PermissionRiskLevel;
  reason: string;
}

function permissionFields(message: ChatMessage) {
  const input =
    message.toolInput && typeof message.toolInput === "object"
      ? (message.toolInput as Record<string, unknown>)
      : {};
  const stringValue = (...keys: string[]) =>
    keys
      .map((key) => input[key])
      .find(
        (value): value is string =>
          typeof value === "string" && value.trim().length > 0,
      )
      ?.trim() ?? "";
  return {
    tool: message.toolName?.trim() ?? "",
    command: stringValue("command", "cmd", "script"),
    cwd: stringValue("cwd", "working_directory", "workingDirectory"),
  };
}

const HIGH_RISK_PATTERNS: Array<[RegExp, string]> = [
  [/\brm\s+(?:-[^\s]*\s+)*-r[fF]/i, "可能删除大量文件"],
  [/(?:^|[\s;&|])rm\s+--no-preserve-root/i, "可能删除系统文件"],
  [/(?:^|[\s;&|])(?:del|erase)\s+\/f/i, "可能强制删除文件"],
  [/(?:^|[\s;&|])(?:rmdir|rd)\s+\/s/i, "可能删除整个目录"],
  [/\bformat\s+[a-z]:/i, "可能格式化磁盘"],
  [
    /\b(?:shutdown|reboot|restart-computer|stop-computer)\b/i,
    "可能影响系统运行",
  ],
  [/(?:^|[\s;&|])sudo\b/i, "需要系统管理员权限"],
  [/(?:^|[\s;&|])runas\b/i, "需要切换系统权限"],
  [/start-process[\s\S]*-verb\s+runas/i, "需要系统管理员权限"],
  [
    /\bgit\s+(?:push|reset\s+--hard|clean\s+-[a-z]*f|checkout\s+--)/i,
    "可能破坏或发布代码",
  ],
  [/(?:drop\s+(?:database|table)|truncate\s+table)/i, "可能破坏数据库数据"],
  [
    /(?:^|[\s;&|])(?:curl|wget|invoke-webrequest|invoke-restmethod)\b/i,
    "可能访问或上传外部网络",
  ],
  [/(?:^|[\s;&|])(?:scp|ssh)\b/i, "可能访问外部机器"],
  [
    /(?:^|[\s;&|])(?:npm|pnpm|yarn|cargo)\s+publish\b/i,
    "可能向公共仓库发布内容",
  ],
  [/(?:^|[\s;&|])(?:chmod\s+777|chown\b)/i, "可能修改系统文件权限"],
];

export function classifyPermissionRisk(message: ChatMessage): PermissionRisk {
  const { tool, command } = permissionFields(message);
  const haystack = [tool, command].filter(Boolean).join(" ");
  if (!tool && !command) {
    return { level: "high", reason: "请求信息不完整，无法安全判断" };
  }
  const knownReadOnlyTool =
    /\b(?:read|glob|grep|search|list|view|inspect)\b/i.test(tool);
  if (!command && !knownReadOnlyTool) {
    return { level: "high", reason: "没有明确命令，需人工确认实际操作" };
  }
  const toolLooksDestructive =
    /\b(?:delete|remove|destroy|kill|shutdown|format|publish|upload)\b/i.test(
      tool,
    );
  if (toolLooksDestructive) {
    return {
      level: "high",
      reason: "工具名称表明它可能会删除、发布或中断资源",
    };
  }
  for (const [pattern, reason] of HIGH_RISK_PATTERNS) {
    if (pattern.test(haystack)) return { level: "high", reason };
  }
  return {
    level: "low",
    reason: "未检测到删除、提权、发布或外部传输特征",
  };
}

export function isHighRiskPermission(message: ChatMessage) {
  return classifyPermissionRisk(message).level === "high";
}
