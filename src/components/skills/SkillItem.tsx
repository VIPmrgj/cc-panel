import { AlertTriangle, Check, ChevronDown, Eye, Power } from "lucide-react";
import type { SkillOverrideSelection, SkillRecord } from "../../api/dto";
import { Button } from "../common/Button";

interface Props {
  skill: SkillRecord;
  selected: boolean;
  pending: boolean;
  onToggleSelected: () => void;
  onChangeState: (value: Exclude<SkillOverrideSelection, "unknown">) => void;
  onPreview: () => void;
}

const labels: Record<Exclude<SkillOverrideSelection, "unknown">, string> = {
  default: "继承",
  on: "开启",
  "name-only": "仅名称",
  "user-invocable-only": "仅手动",
  off: "关闭",
};

export function SkillItem({
  skill,
  selected,
  pending,
  onToggleSelected,
  onChangeState,
  onPreview,
}: Props) {
  const effectivelyOff = skill.overrideState === "off";
  return (
    <article className="skill-item" data-off={effectivelyOff || undefined}>
      <div className="skill-item__topline">
        <label className="skill-check">
          <input
            type="checkbox"
            checked={selected}
            disabled={effectivelyOff || pending || !skill.manifestHash}
            onChange={onToggleSelected}
          />
          <span className="skill-check__box" aria-hidden="true">
            {selected && <Check size={12} />}
          </span>
          <span className="skill-item__name">{skill.displayName}</span>
        </label>
        <Button
          variant="ghost"
          className="icon-button icon-button--small"
          aria-label={`预览 ${skill.displayName}`}
          title="预览 SKILL.md"
          icon={<Eye size={14} />}
          onClick={onPreview}
        >
          <span className="sr-only">预览</span>
        </Button>
      </div>
      <p className="skill-item__description">
        {skill.description || "没有可用描述"}
      </p>
      <div className="skill-item__meta">
        <span className={`source-badge source-badge--${skill.source}`}>
          {skill.sourceLabel}
        </span>
        {skill.collisionInstanceIds.length > 0 && (
          <span className="warning-badge" title="存在同名来源冲突">
            <AlertTriangle size={12} aria-hidden="true" />
            冲突
          </span>
        )}
        {!skill.modelInvocable && (
          <span className="muted-badge">不自动调用</span>
        )}
      </div>
      <div className="skill-item__actions">
        <button
          className="power-toggle"
          data-on={!effectivelyOff || undefined}
          disabled={pending || skill.overrideState === "unknown"}
          aria-pressed={!effectivelyOff}
          aria-label={`${effectivelyOff ? "启用" : "关闭"} ${skill.displayName}`}
          onClick={() => onChangeState(effectivelyOff ? "default" : "off")}
        >
          <Power size={13} aria-hidden="true" />
          {effectivelyOff ? "已关闭" : "可用"}
        </button>
        <label className="compact-select-wrap">
          <span className="sr-only">{skill.displayName} 详细状态</span>
          <select
            className="compact-select"
            value={skill.overrideState}
            disabled={pending}
            onChange={(event) =>
              onChangeState(
                event.target.value as Exclude<
                  SkillOverrideSelection,
                  "unknown"
                >,
              )
            }
          >
            {(Object.keys(labels) as Array<keyof typeof labels>).map(
              (value) => (
                <option value={value} key={value}>
                  {labels[value]}
                </option>
              ),
            )}
            {skill.overrideState === "unknown" && (
              <option value="unknown" disabled>
                未知值
                {skill.rawOverrideValue ? `：${skill.rawOverrideValue}` : ""}
              </option>
            )}
          </select>
          <ChevronDown size={12} aria-hidden="true" />
        </label>
      </div>
    </article>
  );
}
