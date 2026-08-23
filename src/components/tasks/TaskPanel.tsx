import { useState } from "react";
import { ListChecks } from "lucide-react";
import { TASK_TEMPLATES, type TaskTemplate } from "./taskTemplates";

interface Props {
  busy?: boolean;
  onRun: (template: TaskTemplate, details?: string) => void;
}

export function TaskPanel({ busy = false, onRun }: Props) {
  const [details, setDetails] = useState<Record<string, string>>({});
  return (
    <section className="task-panel" aria-labelledby="task-panel-title">
      <div className="context-panel__header">
        <div>
          <p className="panel-eyebrow">STARTER TASKS</p>
          <h2 id="task-panel-title">任务</h2>
          <p className="task-panel__intro">
            这些是可直接开始的示例，也可以补充具体目标、报错或文件范围。
          </p>
        </div>
      </div>
      <div className="task-list">
        {TASK_TEMPLATES.map(
          ({ id, title, description, prompt, icon: Icon }) => {
            const extra = details[id] ?? "";
            return (
              <article className="task-card" key={id}>
                <div className="task-card__icon" aria-hidden="true">
                  <Icon size={17} />
                </div>
                <div className="task-card__body">
                  <h3>{title}</h3>
                  <p>{description}</p>
                  <details className="task-card__details">
                    <summary>查看示例指令</summary>
                    <p>{prompt}</p>
                  </details>
                  <label className="task-card__input">
                    <span>补充说明（可选）</span>
                    <textarea
                      value={extra}
                      onChange={(event) =>
                        setDetails((current) => ({
                          ...current,
                          [id]: event.target.value,
                        }))
                      }
                      placeholder="例如：具体报错、目标文件或验收标准"
                      rows={2}
                    />
                  </label>
                  <button
                    type="button"
                    className="button button--secondary task-card__button"
                    disabled={busy}
                    onClick={() =>
                      extra.trim()
                        ? onRun(
                            { id, title, description, prompt, icon: Icon },
                            extra.trim(),
                          )
                        : onRun({ id, title, description, prompt, icon: Icon })
                    }
                  >
                    <ListChecks size={14} aria-hidden="true" />
                    开始这个任务
                  </button>
                </div>
              </article>
            );
          },
        )}
      </div>
    </section>
  );
}
