import { useEffect, useRef, useState } from "react";
import {
  Check,
  CheckCircle2,
  CircleDashed,
  FilePlus2,
  LogOut,
  Play,
  ShieldCheck,
  Workflow,
} from "lucide-react";
import type { DemoRunResult } from "../../api/dto";

interface Props {
  onRunSandbox: (userId: string) => Promise<DemoRunResult>;
  onExit: () => void;
  onEnterRealAgent: () => void;
}

type DemoStatus = "idle" | "running" | "completed" | "error";
type StepState = "pending" | "active" | "done";

const DEMO_STEPS = [
  { title: "接收体验信息", tool: "界面输入", icon: Workflow },
  { title: "展示固定任务步骤", tool: "预设流程", icon: Check },
  { title: "创建安全示例文件", tool: "沙盒文件写入", icon: FilePlus2 },
  { title: "展示文件结果与预览", tool: "结果预览", icon: CheckCircle2 },
];

export function DemoPanel({ onRunSandbox, onExit, onEnterRealAgent }: Props) {
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

  const runDemo = async () => {
    const value = userId.trim();
    if (!value) {
      setError("请先输入名字或用户 ID。");
      return;
    }
    setError("");
    setResult(null);
    setStatus("running");
    setStep(0);

    await wait(350);
    if (!mountedRef.current) return;
    setStep(1);
    await wait(450);
    if (!mountedRef.current) return;
    setStep(2);

    try {
      const next = await onRunSandbox(value);
      if (!mountedRef.current) return;
      setResult(next);
      setStep(3);
      await wait(350);
      if (!mountedRef.current) return;
      setStatus("completed");
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
          <p className="panel-eyebrow">NO API · SAFE SANDBOX</p>
          <h2 id="demo-panel-title">演示模式</h2>
          <p className="demo-panel__subtitle">
            用一个固定的本地流程了解 Agent 如何接收任务、执行步骤并产出文件。
          </p>
        </div>
        <button
          type="button"
          className="panel-icon-button"
          aria-label="退出演示模式"
          title="退出演示模式"
          onClick={onExit}
        >
          <LogOut size={16} aria-hidden="true" />
        </button>
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
            if (status !== "running") void runDemo();
          }}
        >
          <label htmlFor="demo-user-id">先输入你的名字或用户 ID</label>
          <span>它只用于生成演示文件名和文件内容，不会发送到任何模型。</span>
          <input
            id="demo-user-id"
            value={userId}
            maxLength={48}
            disabled={status === "running"}
            onChange={(event) => setUserId(event.target.value)}
            placeholder="例如：小明 或 user-001"
          />
          <div className="demo-input-card__actions">
            <button
              type="submit"
              className="button button--primary"
              disabled={status === "running"}
            >
              <Play size={15} aria-hidden="true" />
              {status === "running" ? "演示进行中…" : "开始沙盒演示"}
            </button>
            {status !== "idle" && status !== "running" && (
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
              {status === "running"
                ? "执行中"
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
                    ) : state === "active" ? (
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
            {status === "running"
              ? "正在执行第 " +
                Math.min(step + 1, DEMO_STEPS.length) +
                " 步，共 " +
                DEMO_STEPS.length +
                " 步。"
              : status === "completed"
                ? "固定流程已执行完毕，下面可以查看生成结果。"
                : status === "error"
                  ? "没有调用模型；仅本地沙盒文件写入失败，可以安全重试。"
                  : "点击“开始沙盒演示”后，这些步骤会按固定顺序执行。"}
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
              <span className="demo-result__path">{result.relativePath}</span>
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
              文件保存在 CC Panel 自己的应用数据目录中，不在你选择的项目目录里。
            </p>
            <div className="demo-result__actions">
              <button
                type="button"
                className="button button--primary"
                onClick={onEnterRealAgent}
              >
                了解真实 Agent 配置
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={onExit}
              >
                返回聊天
              </button>
            </div>
          </section>
        )}

        <p className="demo-panel__footer">
          你可以随时退出演示。只有进入真实 Agent
          并主动发送消息后，才可能调用模型或执行项目操作。
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
  if (status === "running" && index === activeStep) return "active";
  return "pending";
}

function wait(milliseconds: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, milliseconds);
  });
}
