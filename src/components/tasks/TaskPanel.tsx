import { useState } from "react";
import { ListChecks, PlayCircle, ShieldCheck } from "lucide-react";
import { TASK_TEMPLATES, type TaskTemplate } from "./taskTemplates";

interface Props {
  busy?: boolean;
  onRun: (template: TaskTemplate, details?: string) => void;
  onOpenDemo: () => void;
}

export function TaskPanel({ busy = false, onRun, onOpenDemo }: Props) {
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
          <div className="task-demo-callout">
            <ShieldCheck size={18} aria-hidden="true" />
            <div>
              <strong>没有 API Key？先体验演示模式</strong>
              <p>
                不调用模型、不读取真实项目，只在 CC Panel
                沙盒里完成一个固定流程。
              </p>
              <button
                type="button"
                className="button button--secondary"
                onClick={onOpenDemo}
              >
                <PlayCircle size={14} aria-hidden="true" />
                开始沙盒演示
              </button>
            </div>
          </div>
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
