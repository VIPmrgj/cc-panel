import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../api/dto";
import { classifyPermissionRisk } from "./permissionRisk";

function permission(toolName: string, command?: string): ChatMessage {
  return {
    id: "permission-1",
    role: "permission",
    content: "",
    toolName,
    toolInput: command ? { command } : {},
  };
}

describe("permissionRisk", () => {
  it("classifies ordinary project commands as low risk", () => {
    expect(classifyPermissionRisk(permission("Bash", "npm test")).level).toBe(
      "low",
    );
    expect(classifyPermissionRisk(permission("Read")).level).toBe("low");
  });

  it("requires confirmation for destructive, privileged, and external commands", () => {
    expect(
      classifyPermissionRisk(permission("Bash", "rm -rf build")).level,
    ).toBe("high");
    expect(
      classifyPermissionRisk(permission("Bash", "sudo npm install")).level,
    ).toBe("high");
    expect(
      classifyPermissionRisk(permission("Bash", "git push origin main")).level,
    ).toBe("high");
  });

  it("keeps incomplete unknown requests manual", () => {
    expect(classifyPermissionRisk(permission("UnknownTool")).level).toBe(
      "high",
    );
  });
});
