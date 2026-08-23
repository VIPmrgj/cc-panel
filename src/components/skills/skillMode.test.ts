import { describe, expect, it } from "vitest";
import type { SkillRecord } from "../../api/dto";
import { getBasicDefaultSkills } from "./skillMode";

function skill(overrides: Partial<SkillRecord> = {}): SkillRecord {
  return {
    instanceId: "user:test",
    canonicalId: "test",
    displayName: "Test",
    description: "普通 Skill",
    source: "user",
    sourceLabel: "用户",
    manifestPath: "C:/skills/test/SKILL.md",
    manifestHash: "hash",
    manifestPreview: "# Test",
    overrideState: "default",
    explicitOverride: false,
    userInvocable: true,
    modelInvocable: true,
    collisionInstanceIds: [],
    warnings: [],
    ...overrides,
  };
}

describe("skillMode", () => {
  it("selects explicit and manifest-declared defaults only", () => {
    const defaults = getBasicDefaultSkills([
      skill({
        instanceId: "declared",
        description: "SDD 默认开启，始终开启。",
      }),
      skill({
        instanceId: "explicit",
        overrideState: "on",
      }),
      skill({
        instanceId: "ordinary",
      }),
      skill({
        instanceId: "disabled",
        overrideState: "off",
      }),
      skill({
        instanceId: "missing",
        manifestHash: "",
        description: "默认开启",
      }),
    ]);

    expect(defaults.map((item) => item.instanceId)).toEqual([
      "declared",
      "explicit",
    ]);
  });
});
