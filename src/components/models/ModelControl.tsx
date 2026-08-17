import { useEffect, useId, useState } from "react";
import { Save, Trash2 } from "lucide-react";
import type { ModelStatus } from "../../api/dto";
import { Button } from "../common/Button";
import { Notice } from "../common/Notice";

interface Props {
  model: ModelStatus;
  saving: boolean;
  onSave: (model: string) => void;
  onClear: () => void;
}

export function ModelControl({ model, saving, onSave, onClear }: Props) {
  const [value, setValue] = useState(model.desiredUserModel ?? "");
  const sectionTitleId = useId();
  const inputId = useId();
  useEffect(
    () => setValue(model.desiredUserModel ?? ""),
    [model.desiredUserModel],
  );

  return (
    <section className="sidebar-section" aria-labelledby={sectionTitleId}>
      <div className="section-heading">
        <div>
          <p className="section-kicker">CONFIG</p>
          <h2 id={sectionTitleId}>默认模型</h2>
        </div>
      </div>
      <label className="field-label" htmlFor={inputId}>
        期望的用户默认模型
      </label>
      <input
        id={inputId}
        className="text-input"
        value={value}
        disabled={saving}
        onChange={(event) => setValue(event.target.value)}
        placeholder="opus 或自定义模型 ID"
        spellCheck={false}
      />
      <p className="field-help">
        只更新用户 settings.json 顶层 model，未知 ID 原样保留。
      </p>
      <div className="button-row">
        <Button
          variant="primary"
          icon={<Save size={15} />}
          busy={saving}
          disabled={!value || value === model.desiredUserModel}
          onClick={() => onSave(value)}
        >
          保存
        </Button>
        <Button
          variant="ghost"
          icon={<Trash2 size={15} />}
          disabled={saving || !model.desiredUserModel}
          onClick={onClear}
        >
          清除
        </Button>
      </div>
      {model.candidates.length > 0 && (
        <div className="candidate-list" aria-label="检测到的模型覆盖候选">
          <p className="field-label">检测到的覆盖候选</p>
          {model.candidates.map((candidate) => (
            <div
              className="candidate"
              key={`${candidate.source}-${candidate.label}`}
            >
              <span>
                {candidate.label}
                {candidate.enforced && (
                  <strong className="enforced-badge">强制</strong>
                )}
              </span>
              <code>{candidate.value}</code>
            </div>
          ))}
        </div>
      )}
      {model.warnings.map((warning) => (
        <Notice tone="warning" key={warning}>
          {warning}
        </Notice>
      ))}
      <Notice tone="warning">
        当前运行中 Claude 会话的实际模型无法观察；`/model`、`--model`
        或策略可能覆盖此值。
      </Notice>
    </section>
  );
}
