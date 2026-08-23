import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const channelMock = vi.hoisted(
  () =>
    class MockChannel<T> {
      onmessage: (message: T) => void;
      constructor(onmessage?: (message: T) => void) {
        this.onmessage = onmessage ?? (() => undefined);
      }
    },
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: channelMock,
}));

import { commands } from "./commands";

describe("Tauri command contracts", () => {
  it("uses camelCase invoke arguments for settings and attachments", async () => {
    invokeMock.mockResolvedValue(undefined);

    await commands.setSkillOverride("plugin:skill", "name-only", "rev-1");
    expect(invokeMock).toHaveBeenLastCalledWith("set_skill_override", {
      canonicalId: "plugin:skill",
      value: "name-only",
      settingsRevision: "rev-1",
    });

    await commands.confirmSensitiveImport("token-1");
    expect(invokeMock).toHaveBeenLastCalledWith("confirm_sensitive_import", {
      confirmationToken: "token-1",
    });

    await commands.removeAdditionalRoot("root-1");
    expect(invokeMock).toHaveBeenLastCalledWith("remove_additional_root", {
      rootId: "root-1",
    });
  });

  it("sends revision-protected model profile commands", async () => {
    const profile = {
      providerName: "Anthropic",
      baseUrl: "https://api.anthropic.com",
      modelId: "claude-opus-5",
      selected: true,
    };
    invokeMock.mockResolvedValue({
      schemaVersion: 1,
      revision: 2,
      profiles: [],
    });

    await commands.saveModelProfile(profile, 1);
    expect(invokeMock).toHaveBeenLastCalledWith("save_model_profile", {
      profile,
      expectedRevision: 1,
    });

    await commands.promptAndSaveModelProfile(profile, 2);
    expect(invokeMock).toHaveBeenLastCalledWith(
      "prompt_and_save_model_profile",
      {
        profile,
        expectedRevision: 2,
      },
    );

    await commands.deleteModelProfile("profile-1", 2);
    expect(invokeMock).toHaveBeenLastCalledWith("delete_model_profile", {
      profileId: "profile-1",
      expectedRevision: 2,
    });

    await commands.selectModelProfile(null, 3);
    expect(invokeMock).toHaveBeenLastCalledWith("select_model_profile", {
      profileId: null,
      expectedRevision: 3,
    });

    await commands.restoreModelProfileSelection("profile-1", 4);
    expect(invokeMock).toHaveBeenLastCalledWith(
      "restore_model_profile_selection",
      {
        profileId: "profile-1",
        expectedRevision: 4,
      },
    );
  });

  it("wraps Claude session requests and passes the streaming Channel", async () => {
    const channel = { onmessage: vi.fn() };
    const startRequest = {
      mode: "new" as const,
      profileId: "profile-1",
      title: "Demo",
    };
    const composition = {
      originalPrompt: "Inspect the project",
      enhancedPrompt: null,
      useEnhanced: false,
      selectedSkills: [],
      attachmentHandles: [],
    };
    invokeMock.mockResolvedValueOnce({
      sessionId: "session-1",
      runId: "run-1",
      status: "starting",
      autoCompactTokens: 272000,
      compactionObservable: true,
    });
    await commands.startClaudeSession(startRequest, channel as never);
    expect(invokeMock).toHaveBeenLastCalledWith("start_claude_session", {
      request: startRequest,
      channel,
    });

    invokeMock.mockResolvedValueOnce(undefined);
    await commands.sendClaudeMessage({
      sessionId: "session-1",
      runId: "run-1",
      composition,
    });
    expect(invokeMock).toHaveBeenLastCalledWith("send_claude_message", {
      request: { sessionId: "session-1", runId: "run-1", composition },
    });

    invokeMock.mockResolvedValueOnce(undefined);
    await commands.respondToPermission({
      sessionId: "session-1",
      runId: "run-1",
      requestId: "request-1",
      behavior: "allow",
    });
    expect(invokeMock).toHaveBeenLastCalledWith("respond_to_permission", {
      request: {
        sessionId: "session-1",
        runId: "run-1",
        requestId: "request-1",
        behavior: "allow",
      },
    });
  });

  it("keeps the demo command isolated from Claude session requests", async () => {
    invokeMock.mockResolvedValue({
      userId: "小明",
      fileName: "hello_小明.html",
      displayPath: "桌面/hello_小明.html",
      content: "<html></html>",
      createdAtMs: 1,
    });

    await commands.runDemoSandbox("小明");
    expect(invokeMock).toHaveBeenLastCalledWith("run_demo_sandbox", {
      userId: "小明",
    });
  });
  it("wraps composition requests and preserves backend errors", async () => {
    const request = {
      originalPrompt: "test",
      enhancedPrompt: null,
      useEnhanced: false,
      selectedSkills: [],
      attachmentHandles: [],
    };
    invokeMock.mockResolvedValueOnce({ text: "result" });
    await commands.composePreview(request);
    expect(invokeMock).toHaveBeenLastCalledWith("compose_preview", { request });

    invokeMock.mockRejectedValueOnce({
      code: "REVISION_CONFLICT",
      message: "设置已变化。",
      retryable: true,
      field: "settingsRevision",
    });
    await expect(commands.clearUserModel("stale")).rejects.toMatchObject({
      name: "CcPanelError",
      code: "REVISION_CONFLICT",
      message: "设置已变化。",
      retryable: true,
      field: "settingsRevision",
    });
  });
});
