import { describe, expect, it } from "vitest";
import { composerReducer, initialComposerState } from "./composerReducer";
import type { AttachmentRecord, SkillRecord } from "../api/dto";

const skill = {
  instanceId: "skill-1",
  canonicalId: "example",
  displayName: "example",
  description: "Example",
  source: "user",
  sourceLabel: "用户",
  manifestPath: "C:/fixture/SKILL.md",
  manifestHash: "hash",
  manifestPreview: "body",
  overrideState: "default",
  explicitOverride: false,
  userInvocable: true,
  modelInvocable: true,
  collisionInstanceIds: [],
  warnings: [],
} satisfies SkillRecord;

const attachment = {
  handle: "a",
  name: "a.txt",
  path: "C:/fixture/a.txt",
  kind: "text",
  mime: "text/plain",
  rawBytes: 1,
  extractedBytes: 1,
  sha256: "sha",
  warnings: [],
} satisfies AttachmentRecord;

describe("composerReducer", () => {
  it("never overwrites the original prompt with an enhanced candidate", () => {
    const withPrompt = composerReducer(initialComposerState, {
      type: "setOriginalPrompt",
      value: "original",
    });
    const enhanced = composerReducer(withPrompt, {
      type: "setEnhancedPrompt",
      value: "enhanced",
    });
    expect(enhanced.originalPrompt).toBe("original");
    expect(enhanced.enhancedPrompt).toBe("enhanced");
    expect(enhanced.useEnhanced).toBe(true);
  });

  it("reconciles selected skills with a refreshed inventory", () => {
    const selected = composerReducer(initialComposerState, {
      type: "toggleSkill",
      skill,
    });
    const refreshed = {
      ...skill,
      manifestHash: "new-hash",
    };
    const reconciled = composerReducer(selected, {
      type: "reconcileSkills",
      skills: [refreshed],
    });
    expect(reconciled.selectedSkills).toEqual([refreshed]);
    expect(reconciled.previewStale).toBe(true);

    const disabled = composerReducer(reconciled, {
      type: "reconcileSkills",
      skills: [{ ...refreshed, overrideState: "off" }],
    });
    expect(disabled.selectedSkills).toEqual([]);
  });

  it("marks previews stale after any composition input changes", () => {
    let state = composerReducer(initialComposerState, {
      type: "setPreview",
      preview: {
        text: "x",
        compositionId: "id",
        utf8Bytes: 1,
        characters: 1,
        lines: 1,
        skillCount: 0,
        attachmentCount: 0,
        promptVariant: "original",
        warnings: [],
      },
    });
    expect(state.previewStale).toBe(false);
    state = composerReducer(state, { type: "toggleSkill", skill });
    expect(state.previewStale).toBe(true);
    state = composerReducer(state, {
      type: "addAttachments",
      attachments: [attachment],
    });
    expect(state.attachments).toHaveLength(1);
  });
});
