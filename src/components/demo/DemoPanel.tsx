import { useEffect, useRef, useState } from "react";
import {
  Check,
  CheckCircle2,
  CircleDashed,
  FilePlus2,
  Play,
  ShieldCheck,
  Workflow,
} from "lucide-react";
import type { DemoRunResult } from "../../api/dto";

interface Props {
  onRunSandbox: (userId: string) => Promise<DemoRunResult>;
  onCompleted?: () => void;
}

type DemoStatus = "idle" | "planned" | "writing" | "completed" | "error";
type StepState = "pending" | "active" | "done";

const DEMO_STEPS = [
  { title: "接收你的名字或 ID", tool: "界面输入", icon: Workflow },
  { title: "展示 Agent 的固定步骤", tool: "预设流程", icon: Check },
  { title: "在桌面创建示例文件", tool: "桌面文件写入", icon: FilePlus2 },
  { title: "展示文件内容与预览", tool: "结果预览", icon: CheckCircle2 },
];

export function DemoPanel({ onRunSandbox, onCompleted }: Props) {
  const [userId, setUserId] = useState("");
  const [status, setStatus] = useState<DemoStatus>("idle");
  const [step, setStep] = useState(0);
  const [result, setResult] = useState<DemoRunResult | null>(null);
  const [error, setError] = useState("");
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const beginDemo = () => {
    const value = userId.trim();
    if (!value) {
      setError("请先输入名字或用户 ID。");
      return;
    }
    setError("");
    setResult(null);
    setStatus("planned");
    setStep(1);
  };

  const advanceDemo = async () => {
    if (status !== "planned") return;
    if (step === 0) {
      setStep(1);
      return;
    }

    const value = userId.trim();
    setError("");
    setStatus("writing");
    setStep(2);
    try {
      const next = await onRunSandbox(value);
      if (!mountedRef.current) return;
      setResult(next);
      setStep(3);
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

  const resetDemo = () => {
    setStatus("idle");
    setStep(0);
    setResult(null);
    setError("");
  };

  return (
    <section className="demo-panel" aria-labelledby="demo-panel-title">
      <header className="context-panel__header demo-panel__header">
        <div>
          <p className="panel-eyebrow">最后一步 · NO API · SAFE SANDBOX</p>
          <h2 id="demo-panel-title">动手体验 Agent 流程</h2>
          <p className="demo-panel__subtitle">
            这是新手引导的最后一步。你会亲自看到“接收任务 → 展示计划 → 创建文件
            → 查看结果”。
          </p>
        </div>
      </header>

      <div className="demo-panel__body">
        <div className="demo-safety-banner" role="note">
          <ShieldCheck size={19} aria-hidden="true" />
          <div>
            <strong>这是演示模式，不是 AI 对话</strong>
            <p>
              不调用模型、不需要 API
              Key、不读取真实项目、不读取密钥，也不执行任意 Shell 命令。
            </p>
          </div>
        </div>

        <form
          className="demo-input-card"
          onSubmit={(event) => {
            event.preventDefault();
            if (status === "idle" || status === "error") void beginDemo();
            else if (status === "planned") void advanceDemo();
          }}
        >
          <label htmlFor="demo-user-id">先输入你的名字或用户 ID</label>
          <span>
            它只用于生成演示文件名和文件内容，不会发送到任何模型。每一步由你确认后继续。
          </span>
          <input
            id="demo-user-id"
            value={userId}
            maxLength={48}
            disabled={status === "writing" || status === "completed"}
            onChange={(event) => setUserId(event.target.value)}
            placeholder="例如：小明 或 user-001"
          />
          <div className="demo-input-card__actions">
            <button
              type="submit"
              className="button button--primary"
              disabled={status === "writing" || status === "completed"}
            >
              <Play size={15} aria-hidden="true" />
              {status === "writing"
                ? "正在创建桌面文件…"
                : status === "planned"
                  ? "下一步：在桌面创建文件"
                  : status === "completed"
                    ? "演示已完成"
                    : "开始第 1 步"}
            </button>
            {(status === "completed" || status === "error") && (
              <button
                type="button"
                className="button button--ghost"
                onClick={resetDemo}
              >
                再试一次
              </button>
            )}
          </div>
        </form>

        <section
          className="demo-progress-card"
          aria-labelledby="demo-progress-title"
        >
          <div className="demo-section-heading">
            <div>
              <p className="panel-eyebrow">FIXED WORKFLOW</p>
              <h3 id="demo-progress-title">当前任务：生成欢迎示例文件</h3>
            </div>
            <span className={"demo-status demo-status--" + status}>
              {status === "planned"
                ? "等待你继续"
                : status === "writing"
                  ? "正在写入桌面"
                  : status === "completed"
                    ? "已完成"
                    : status === "error"
                      ? "需要重试"
                      : "等待开始"}
            </span>
          </div>
          <ol className="demo-steps">
            {DEMO_STEPS.map(({ title, tool, icon: Icon }, index) => {
              const state = stepState(status, step, index);
              return (
                <li className="demo-step" data-state={state} key={title}>
                  <span className="demo-step__icon" aria-hidden="true">
                    {state === "done" ? (
                      <Check size={15} />
                    ) : state === "active" && status === "writing" ? (
                      <CircleDashed size={15} className="spin" />
                    ) : (
                      <Icon size={15} />
                    )}
                  </span>
                  <span className="demo-step__body">
                    <strong>{title}</strong>
                    <small>工具类型：{tool}</small>
                  </span>
                </li>
              );
            })}
          </ol>
          <p className="demo-progress-card__note" aria-live="polite">
            {status === "planned"
              ? step === 0
                ? "第 1 步已准备好。点击下一步，先看 Agent 会做什么。"
                : "第 2 步已展示。点击下一步后，才会在桌面创建文件。"
              : status === "writing"
                ? "正在执行第 3 步：只写入桌面上的一个固定 HTML 示例文件。"
                : status === "completed"
                  ? "固定流程已执行完毕，下面显示的是真实写入桌面的文件内容。"
                  : status === "error"
                    ? "没有调用模型；桌面文件写入失败，可以安全重试。"
                    : "输入名字或 ID 后，点击“开始第 1 步”查看流程。"}
          </p>
        </section>

        {error && (
          <div className="demo-error" role="alert">
            {error}
          </div>
        )}

        {result && (
          <section className="demo-result" aria-labelledby="demo-result-title">
            <div className="demo-section-heading">
              <div>
                <p className="panel-eyebrow">VERIFIABLE RESULT</p>
                <h3 id="demo-result-title">文件已创建</h3>
              </div>
              <span className="demo-result__path">{result.displayPath}</span>
            </div>
            <div className="demo-result__grid">
              <div>
                <h4>文件内容</h4>
                <pre>{result.content}</pre>
              </div>
              <div>
                <h4>安全预览</h4>
                <iframe
                  title="演示文件预览"
                  className="demo-result__preview"
                  sandbox=""
                  srcDoc={result.content}
                />
              </div>
            </div>
            <p className="demo-result__note">
              文件已经真实写入你的桌面，不在你选择的项目目录里，也没有调用模型或执行外部命令。
            </p>
          </section>
        )}

        <p className="demo-panel__footer">
          演示完成后，请点击引导窗口底部的“完成引导”；如果暂时没有 API
          Key，也可以先跳过后续真实配置。
        </p>
      </div>
    </section>
  );
}

function stepState(
  status: DemoStatus,
  activeStep: number,
  index: number,
): StepState {
  if (status === "completed" || index < activeStep) return "done";
  if ((status === "planned" || status === "writing") && index === activeStep)
    return "active";
  return "pending";
}
