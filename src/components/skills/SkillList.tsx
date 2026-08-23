import { useId, useState } from "react";
import { ChevronDown, Search, RefreshCw } from "lucide-react";
import type { SkillOverrideSelection, SkillRecord } from "../../api/dto";
import { Button } from "../common/Button";
import { Notice } from "../common/Notice";
import { SkillItem } from "./SkillItem";
import type { SkillPanelMode } from "./skillMode";

interface Props {
  skills: SkillRecord[];
  selectedIds: Set<string>;
  search: string;
  refreshing: boolean;
  pending: boolean;
  pluginWarning?: string | null;
  mode: SkillPanelMode;
  onModeChange: (mode: SkillPanelMode) => void;
  onSearch: (value: string) => void;
  onRefresh: () => void;
  onToggleSelected: (skill: SkillRecord) => void;
  onChangeState: (
    skill: SkillRecord,
    value: Exclude<SkillOverrideSelection, "unknown">,
  ) => void;
  onPreview: (skill: SkillRecord) => void;
}

export function SkillList({
  skills,
  selectedIds,
  search,
  refreshing,
  pending,
  pluginWarning,
  mode,
  onModeChange,
  onSearch,
  onRefresh,
  onToggleSelected,
  onChangeState,
  onPreview,
}: Props) {
  const bodyId = useId();
  // The Skills library stays collapsed by default so the sidebar remains compact.
  const [collapsed, setCollapsed] = useState(true);
  const normalized = search.trim().toLocaleLowerCase();
  const visible = normalized
    ? skills.filter((skill) =>
        (skill.displayName + " " + skill.description + " " + skill.sourceLabel)
          .toLocaleLowerCase()
          .includes(normalized),
      )
    : skills;
  const grouped = visible.reduce((groups, skill) => {
    const items = groups.get(skill.source);
    if (items) {
      items.push(skill);
    } else {
      groups.set(skill.source, [skill]);
    }
    return groups;
  }, new Map<SkillRecord["source"], SkillRecord[]>());
  const groupOrder: Array<SkillRecord["source"]> = [
    "project",
    "user",
    "additional",
    "plugin",
  ];
  const groupLabels: Record<SkillRecord["source"], string> = {
    project: "PROJECT",
    user: "USER",
    additional: "ADDITIONAL",
    plugin: "PLUGINS",
  };

  return (
    <section
      className="skills-section"
      aria-label="Skills"
      title="Skill 可以理解为 Agent 的工作说明或额外能力。勾选后，本次消息才会把它带给 Agent。"
    >
      <div className="section-heading section-heading--sticky">
        <button
          type="button"
          className="skills-toggle"
          aria-expanded={!collapsed}
          aria-controls={bodyId}
          onClick={() => setCollapsed((value) => !value)}
        >
          <ChevronDown
            size={14}
            aria-hidden="true"
            className={"skills-toggle__chevron" + (collapsed ? "" : " is-open")}
          />
          <span className="skills-toggle__label">
            <span className="section-kicker">LIBRARY</span>
            <strong>
              Skills <span className="count">{skills.length}</span>
            </strong>
          </span>
        </button>
        <Button
          variant="ghost"
          className="icon-button"
          aria-label="刷新 Skill 清单"
          title="刷新"
          icon={<RefreshCw size={15} />}
          busy={refreshing}
          disabled={pending}
          onClick={onRefresh}
        >
          <span className="sr-only">刷新</span>
        </Button>
      </div>
      {!collapsed && (
        <div id={bodyId}>
          <div
            className="skill-mode-switch"
            role="group"
            aria-label="Skill 使用模式"
          >
            <span
              className="skill-mode-switch__label"
              title="简洁显示只保留勾选；完整控制可以调整 Skill 的加载和调用方式。"
            >
              显示方式
            </span>
            <button
              type="button"
              aria-pressed={mode === "basic"}
              onClick={() => onModeChange("basic")}
            >
              简洁显示
            </button>
            <button
              type="button"
              aria-pressed={mode === "advanced"}
              onClick={() => onModeChange("advanced")}
            >
              完整控制
            </button>
          </div>
          <p className="skill-mode-hint">
            {mode === "basic"
              ? "基础模式：勾选 = 本次消息允许 Agent 使用；不勾选 = 不提供给 Agent。"
              : "高级模式：可以精确控制 Skill 是自动可用、只显示名称、仅手动调用还是完全关闭。"}
          </p>
          {mode === "advanced" && (
            <details className="skill-mode-help">
              <summary>这些状态是什么意思？</summary>
              <ul>
                <li>
                  <strong>继承</strong>：沿用项目或用户的原有设置。
                </li>
                <li>
                  <strong>开启</strong>：允许 Agent 在需要时自动使用。
                </li>
                <li>
                  <strong>仅名称</strong>：只告诉 Agent 有这个
                  Skill，不加载完整内容。
                </li>
                <li>
                  <strong>仅手动</strong>：只有你明确点名时才使用。
                </li>
                <li>
                  <strong>关闭</strong>
                  ：完全不提供；所以即使勾选，关闭状态也会优先生效。
                </li>
              </ul>
            </details>
          )}
          <label className="search-field">
            <Search size={15} aria-hidden="true" />
            <span className="sr-only">搜索 Skills</span>
            <input
              value={search}
              onChange={(event) => onSearch(event.target.value)}
              placeholder="搜索 Skills"
            />
          </label>
          {pluginWarning && <Notice tone="warning">{pluginWarning}</Notice>}
          {visible.length === 0 ? (
            <div className="empty-state compact-empty">没有匹配的 Skill。</div>
          ) : (
            <div className="skill-groups">
              {groupOrder.map((group) => {
                const items = grouped.get(group);
                if (!items?.length) return null;
                return (
                  <section className="skill-group" key={group}>
                    <h3>
                      {groupLabels[group]} <span>{items.length}</span>
                    </h3>
                    <div className="skill-group__items">
                      {items.map((skill) => (
                        <SkillItem
                          key={skill.instanceId}
                          skill={skill}
                          selected={selectedIds.has(skill.instanceId)}
                          pending={pending}
                          advanced={mode === "advanced"}
                          onToggleSelected={() => onToggleSelected(skill)}
                          onChangeState={(value) => onChangeState(skill, value)}
                          onPreview={() => onPreview(skill)}
                        />
                      ))}
                    </div>
                  </section>
                );
              })}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
