import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SkillRecord } from "../../api/dto";
import { SkillList } from "./SkillList";

const skill: SkillRecord = {
  instanceId: "user:test",
  canonicalId: "test",
  displayName: "Test Skill",
  description: "A test Skill",
  source: "user",
  sourceLabel: "User",
  manifestPath: "C:/Users/Test/.claude/skills/test/SKILL.md",
  manifestHash: "abc",
  manifestPreview: "# Test",
  overrideState: "default",
  explicitOverride: false,
  userInvocable: true,
  modelInvocable: true,
  collisionInstanceIds: [],
  warnings: [],
};

describe("SkillList", () => {
  it("serializes refresh and override controls while inventory is busy", async () => {
    const onRefresh = vi.fn();
    const onChangeState = vi.fn();
    const { rerender } = render(
      <SkillList
        skills={[skill]}
        selectedIds={new Set()}
        search=""
        refreshing={false}
        pending
        onSearch={vi.fn()}
        onRefresh={onRefresh}
        onToggleSelected={vi.fn()}
        onChangeState={onChangeState}
        onPreview={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "刷新 Skill 清单" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "关闭 Test Skill" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("combobox", { name: "Test Skill 详细状态" }),
    ).toBeDisabled();

    rerender(
      <SkillList
        skills={[skill]}
        selectedIds={new Set()}
        search=""
        refreshing={false}
        pending={false}
        onSearch={vi.fn()}
        onRefresh={onRefresh}
        onToggleSelected={vi.fn()}
        onChangeState={onChangeState}
        onPreview={vi.fn()}
      />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: "关闭 Test Skill" }),
    );

    expect(onChangeState).toHaveBeenCalledWith(skill, "off");
  });
});
