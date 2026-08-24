import { useEffect, useRef, useState } from "react";
import {
  Check,
  CheckCircle2,
  CircleDashed,
  FilePlus2,
  FolderOpen,
  Play,
  ShieldCheck,
  Sparkles,
  Workflow,
} from "lucide-react";
import type { DemoRunResult } from "../../api/dto";

export interface DemoWizardProps {
  onRunSandbox: (userId: string) => Promise<DemoRunResult>;
  onCompleted?: () => void;
  onSkipped?: () => void;
  onOpenDemoFile?: (fileName: string) => Promise<void>;
}

type DemoStatus = "idle" | "input" | "plan" | "writing" | "completed" | "error";

export function DemoWizard({
  onRunSandbox,
  onCompleted,
  onSkipped,
  onOpenDemoFile,
}: DemoWizardProps) {
  const [userId, setUserId] = useState("");
  const [status, setStatus] = useState<DemoStatus>("idle");
  const [result, setResult] = useState<DemoRunResult | null>(null);
  const [error, setError] = useState("");
  const [openMessage, setOpenMessage] = useState("");
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const beginDemo = () => {
    setError("");
    setOpenMessage("");
    setStatus("input");
  };

  const submitUserId = () => {
    if (!userId.trim()) {
      setError("请先输入名字或用户 ID。");
      return;
    }
    setError("");
    setOpenMessage("");
    setStatus("plan");
  };

  const createDemoFile = async () => {
    const value = userId.trim();
    if (!value) {
      setStatus("input");
      setError("请先输入名字或用户 ID。");
      return;
    }
    setError("");
    setOpenMessage("");
    setStatus("writing");
    try {
      const next = await onRunSandbox(value);
      if (!mountedRef.current) return;
      setResult(next);
      setStatus("completed");
      onCompleted?.();
    } catch (runError) {
      if (!mountedRef.current) return;
      setStatus("error");
      setError(
        runError instanceof Error
          ? runError.message
          : "演示文件创建失败，请重试。",
      );
    }
  };

  const openDemoFile = async () => {
    if (!result || !onOpenDemoFile) return;
    setOpenMessage("");
    try {
      await onOpenDemoFile(result.fileName);
      if (mountedRef.current) {
        setOpenMessage("桌面文件夹已打开，请找到高亮的演示文件。");
      }
    } catch (openError) {
      if (mountedRef.current) {
        setOpenMessage(
          openError instanceof Error
            ? openError.message
            : "无法打开桌面文件夹，请手动查看桌面。",
        );
      }
    }
  };

  const resetDemo = () => {
    setResult(null);
    setError("");
    setOpenMessage("");
    setStatus("input");
  };

  const step =
    status === "idle"
      ? "准备"
      : status === "input"
        ? "第 1 步 / 4"
        : status === "plan"
          ? "第 2 步 / 4"
          : status === "writing"
            ? "第 3 步 / 4"
            : "第 4 步 / 4";

  return (
    <section
      className="demo-panel"
      data-demo-status={status}
      aria-labelledby="demo-panel-title"
    >
      <header className="context-panel__header demo-panel__header">
        <div>
          <p className="panel-eyebrow">最后一步 · NO API · SAFE SANDBOX</p>
          <h2 id="demo-panel-title">动手体验 Agent 流程</h2>
          <p className="demo-panel__subtitle">
            不会调用模型。你会一步一步看到任务、计划、桌面文件和最终结果。
          </p>
        </div>
        <span className="demo-status">{step}</span>
      </header>

      <div className="demo-panel__body">
        <div className="demo-safety-banner" role="note">
          <ShieldCheck size={19} aria-hidden="true" />
          <div>
            <strong>这是演示模式，不是 AI 对话</strong>
            <p>
              不需要 API Key，不读取真实项目，不读取密钥，也不执行任意 Shell
              命令。
            </p>
          </div>
        </div>

        {status === "idle" && (
          <div className="demo-step-window demo-step-window--choice">
            <Sparkles
              size={28}
              aria-hidden="true"
              className="demo-step-window__hero-icon"
            />
            <p className="panel-eyebrow">OPTIONAL DEMO</p>
            <h3>是否现在进入新手演示？</h3>
            <p>
              如果你还没有 API Key，也可以先用这个安全演示理解 Agent
              是怎样接收任务、执行步骤并产出文件的。
            </p>
            <div className="demo-step-window__actions">
              <button
                type="button"
                className="button button--primary"
                onClick={beginDemo}
              >
                <Play size={16} aria-hidden="true" />
                进入演示
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={onSkipped}
              >
                暂时跳过
              </button>
            </div>
          </div>
        )}

        {status === "input" && (
          <form
            className="demo-step-window"
            onSubmit={(event) => {
              event.preventDefault();
              submitUserId();
            }}
          >
            <StepHeading
              icon={Workflow}
              eyebrow="第 1 步 · 接收任务"
              title="告诉我你的名字或用户 ID"
              description="它只用于生成演示文件名和文件内容，不会发送给任何模型。"
            />
            <label htmlFor="demo-user-id">名字或用户 ID</label>
            <input
              id="demo-user-id"
              value={userId}
              maxLength={48}
              onChange={(event) => setUserId(event.target.value)}
              placeholder="例如：小明 或 user-001"
              autoFocus
            />
            {error && (
              <p className="demo-inline-error" role="alert">
                {error}
              </p>
            )}
            <div className="demo-step-window__actions">
              <button type="submit" className="button button--primary">
                下一步：查看演示计划
              </button>
            </div>
          </form>
        )}

        {status === "plan" && (
          <div className="demo-step-window">
            <StepHeading
              icon={Check}
              eyebrow="第 2 步 · 展示计划"
              title="Agent 将按这 3 步完成任务"
              description="这是预先准备好的固定流程，不会调用真实模型。"
            />
            <ol className="demo-plan-list">
              <li>
                <Check size={17} aria-hidden="true" />
                接收你的名字或用户 ID
              </li>
              <li>
                <FilePlus2 size={17} aria-hidden="true" />在 Windows
                桌面创建一个欢迎 HTML 文件
              </li>
              <li>
                <CheckCircle2 size={17} aria-hidden="true" />
                返回文件结果，并带你查看桌面
              </li>
            </ol>
            <div className="demo-step-window__actions">
              <button
                type="button"
                className="button button--primary"
                onClick={() => void createDemoFile()}
              >
                <FilePlus2 size={16} aria-hidden="true" />
                下一步：创建桌面文件
              </button>
            </div>
          </div>
        )}

        {status === "writing" && (
          <div
            className="demo-step-window demo-step-window--writing"
            role="status"
            aria-live="polite"
          >
            <CircleDashed size={34} className="spin" aria-hidden="true" />
            <p className="panel-eyebrow">第 3 步 · 桌面文件写入</p>
            <h3>正在桌面创建示例文件</h3>
            <p>
              只会写入一个固定的 HTML 文件，不读取你的项目，也不会产生 API
              费用。
            </p>
          </div>
        )}

        {status === "completed" && result && (
          <div className="demo-step-window demo-step-window--completed">
            <CheckCircle2
              size={38}
              aria-hidden="true"
              className="demo-completion-icon"
            />
            <p className="panel-eyebrow">第 4 步 · 查看结果</p>
            <h3>恭喜你，你完成了演示！</h3>
            <p>现在请看看你的桌面，新增的文件就在这里：</p>
            <div className="demo-file-callout">
              <FolderOpen size={20} aria-hidden="true" />
              <code>{result.fileName}</code>
            </div>
            <p className="demo-completion-note">
              点击下面的按钮，CC Panel 会打开桌面文件夹并高亮这个文件。
            </p>
            <div className="demo-step-window__actions">
              <button
                type="button"
                className="button button--primary"
                onClick={() => void openDemoFile()}
              >
                <FolderOpen size={16} aria-hidden="true" />
                打开桌面查看文件
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={resetDemo}
              >
                再看一遍
              </button>
            </div>
            {openMessage && (
              <p className="demo-open-message" role="status">
                {openMessage}
              </p>
            )}
            <details className="demo-result-details">
              <summary>查看文件内容</summary>
              <pre>{result.content}</pre>
            </details>
          </div>
        )}

        {status === "error" && (
          <div
            className="demo-step-window demo-step-window--error"
            role="alert"
          >
            <p className="panel-eyebrow">演示没有完成</p>
            <h3>桌面文件创建失败</h3>
            <p>{error || "请重试，演示不会调用模型。"}</p>
            <div className="demo-step-window__actions">
              <button
                type="button"
                className="button button--primary"
                onClick={() => {
                  setError("");
                  setStatus("plan");
                }}
              >
                重试创建文件
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={() => {
                  setError("");
                  setStatus("input");
                }}
              >
                返回上一步
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function StepHeading({
  icon: Icon,
  eyebrow,
  title,
  description,
}: {
  icon: typeof Workflow;
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div className="demo-step-window__heading">
      <span className="demo-step-window__icon" aria-hidden="true">
        <Icon size={21} />
      </span>
      <div>
        <p className="panel-eyebrow">{eyebrow}</p>
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </div>
  );
}
