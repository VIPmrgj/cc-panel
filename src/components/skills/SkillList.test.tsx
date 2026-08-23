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

function renderExpanded(props: Partial<Parameters<typeof SkillList>[0]> = {}) {
  const result = render(
    <SkillList
      skills={[skill]}
      selectedIds={new Set()}
      search=""
      refreshing={false}
      pending={false}
      mode="basic"
      onModeChange={vi.fn()}
      onSearch={vi.fn()}
      onRefresh={vi.fn()}
      onToggleSelected={vi.fn()}
      onChangeState={vi.fn()}
      onPreview={vi.fn()}
      {...props}
    />,
  );
  return result;
}

describe("SkillList", () => {
  it("is collapsed by default and expands on the header toggle", async () => {
    renderExpanded();
    expect(screen.getByRole("button", { name: /Skills/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );

    await userEvent.click(screen.getByRole("button", { name: /Skills/ }));
    expect(screen.getByRole("button", { name: "简洁显示" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      screen.getByRole("checkbox", { name: "Test Skill" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Skills/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  it("keeps advanced controls hidden in basic mode", async () => {
    const onModeChange = vi.fn();
    const { rerender } = renderExpanded({
      mode: "basic",
      onModeChange,
    });

    await userEvent.click(screen.getByRole("button", { name: /Skills/ }));
    expect(
      screen.queryByRole("combobox", { name: "Test Skill 详细状态" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "预览 Test Skill" }),
    ).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "完整控制" }));
    expect(onModeChange).toHaveBeenCalledWith("advanced");

    rerender(
      <SkillList
        skills={[skill]}
        selectedIds={new Set()}
        search=""
        refreshing={false}
        pending={false}
        mode="advanced"
        onModeChange={onModeChange}
        onSearch={vi.fn()}
        onRefresh={vi.fn()}
        onToggleSelected={vi.fn()}
        onChangeState={vi.fn()}
        onPreview={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("combobox", { name: "Test Skill 详细状态" }),
    ).toBeInTheDocument();
  });

  it("serializes refresh and override controls while inventory is busy", async () => {
    const onRefresh = vi.fn();
    const onChangeState = vi.fn();
    const { rerender } = renderExpanded({
      pending: true,
      mode: "advanced",
      onRefresh,
    });

    await userEvent.click(screen.getByRole("button", { name: /Skills/ }));
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
        mode="advanced"
        onModeChange={vi.fn()}
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
