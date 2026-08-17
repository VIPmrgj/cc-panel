import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

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
