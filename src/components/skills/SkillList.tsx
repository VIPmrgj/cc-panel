import { useId } from "react";
import { Search, RefreshCw } from "lucide-react";
import type { SkillOverrideSelection, SkillRecord } from "../../api/dto";
import { Button } from "../common/Button";
import { Notice } from "../common/Notice";
import { SkillItem } from "./SkillItem";

interface Props {
  skills: SkillRecord[];
  selectedIds: Set<string>;
  search: string;
  refreshing: boolean;
  pending: boolean;
  pluginWarning?: string | null;
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
  onSearch,
  onRefresh,
  onToggleSelected,
  onChangeState,
  onPreview,
}: Props) {
  const titleId = useId();
  const normalized = search.trim().toLocaleLowerCase();
  const visible = normalized
    ? skills.filter((skill) =>
        `${skill.displayName} ${skill.description} ${skill.sourceLabel}`
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
    <section className="skills-section" aria-labelledby={titleId}>
      <div className="section-heading section-heading--sticky">
        <div>
          <p className="section-kicker">LIBRARY</p>
          <h2 id={titleId}>
            Skills <span className="count">{skills.length}</span>
          </h2>
        </div>
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
    </section>
  );
}
