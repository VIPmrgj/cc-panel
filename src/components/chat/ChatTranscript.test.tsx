import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../../api/dto";
import { ChatTranscript } from "./ChatTranscript";
import { extractDisplayedUserPrompt } from "./promptDisplay";

describe("extractDisplayedUserPrompt", () => {
  it("shows only the user prompt while preserving XML transport content", () => {
    const wrapped = [
      '<cc-panel-prompt version="1">',
      "  <selected-skills>",
      '    <skill id="demo">hidden skill body</skill>',
      "  </selected-skills>",
      '  <user-prompt variant="original">',
      "    这是一个测试对话，进行一些权限的提交，然后进行一些测试类对话",
      "    第二行 &lt;保持&gt; &amp; 原样",
      "  </user-prompt>",
      "  <attachments>",
      '    <attachment name="secret.txt">hidden attachment</attachment>',
      "  </attachments>",
      "</cc-panel-prompt>",
    ].join(String.fromCharCode(10));

    expect(extractDisplayedUserPrompt(wrapped)).toBe(
      [
        "这是一个测试对话，进行一些权限的提交，然后进行一些测试类对话",
        "第二行 <保持> & 原样",
      ].join(String.fromCharCode(10)),
    );
    expect(wrapped).toContain("<selected-skills>");
    expect(wrapped).toContain("<attachments>");
  });

  it("leaves legacy plain messages unchanged", () => {
    expect(extractDisplayedUserPrompt("普通历史消息")).toBe("普通历史消息");
  });
});

function permissionMessage(
  requestId: string,
  status: ChatMessage["status"] = "pending",
): ChatMessage {
  return {
    id: "permission-" + requestId,
    role: "permission",
    content: "",
    requestId,
    toolName: "Bash",
    toolInput: {
      command: "powershell -NoProfile -Command " + requestId,
      cwd: "E:\\\\WORK\\\\cc-panel-real-test",
    },
    permissionExpiresAt: Date.now() + 120_000,
    status,
  };
}

describe("ChatTranscript permission controls", () => {
  it("only disables the permission request currently being submitted", () => {
    render(
      <ChatTranscript
        messages={[
          permissionMessage("request-1"),
          permissionMessage("request-2"),
        ]}
        busyPermissionIds={new Set(["request-1"])}
        onPermission={() => undefined}
        onRetryPermission={() => undefined}
      />,
    );

    const allowButtons = screen.getAllByRole("button", { name: "允许一次" });
    expect(allowButtons).toHaveLength(2);
    expect(allowButtons[0]).toBeDisabled();
    expect(allowButtons[1]).not.toBeDisabled();
  });

  it("does not render action buttons for resolved permission history", () => {
    render(
      <ChatTranscript
        messages={[permissionMessage("resolved", "complete")]}
        onPermission={() => undefined}
        onRetryPermission={() => undefined}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "允许一次" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("权限中心")).not.toBeInTheDocument();
  });
});
