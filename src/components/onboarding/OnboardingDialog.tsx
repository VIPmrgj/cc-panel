import {
  Check,
  ChevronLeft,
  ChevronRight,
  Clipboard,
  FolderOpen,
  KeyRound,
  Play,
  RotateCw,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { OllamaStatus } from "../../api/dto";
import type { ExperienceMode } from "../../state/experienceMode";
import { Button } from "../common/Button";

interface Props {
  open: boolean;
  claudeCliAvailable: boolean;
  projectLabel: string | null;
  modelReady: boolean;
  experienceMode: ExperienceMode;
  ollama: OllamaStatus;
  busy: boolean;

  ollamaSaving?: boolean;
  onExperienceModeChange: (mode: ExperienceMode) => void;
  onCopyInstallCommand: () => void;
  onRecheckClaude: () => void;
  onSelectProject: () => void;
  onAddModel: () => void;
  onSelectOllamaModel: (model: string | null) => void;
  onOpenDemo: () => void;
  onClose: () => void;
}

export function OnboardingDialog({
  open,
  claudeCliAvailable,
  projectLabel,
  modelReady,
  experienceMode,
  ollama,
  busy,

  ollamaSaving = false,
  onExperienceModeChange,
  onCopyInstallCommand,
  onRecheckClaude,
  onSelectProject,
  onAddModel,
  onSelectOllamaModel,
  onOpenDemo,
  onClose,
}: Props) {
  const titleId = useId();
  const descriptionId = useId();
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const [step, setStep] = useState(0);
  const totalSteps = 6;
  const stepLabels = [
    "选择体验方式",
    "检查 Claude Code",
    "选择工作文件夹",
    "选择默认模型",
    "本地 Prompt 优化",
    "演示模式",
  ];

  useEffect(() => {
    if (open) setStep(0);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        closeRef.current();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open, busy]);

  if (!open) return null;

  return (
    <div className="modal-backdrop">
      <section
        className="model-dialog onboarding-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <header className="model-dialog__header">
          <div className="model-dialog__title">
            <span className="model-dialog__icon" aria-hidden="true">
              <Sparkles size={17} />
            </span>
            <div>
              <h2 id={titleId}>开始使用 CC Panel</h2>
              <p id={descriptionId}>
                我们一次只处理一件事；每一步都可以跳过，之后也能在设置里重新打开。
              </p>
            </div>
          </div>
        </header>

        <div
          className="onboarding-mode-step"
          data-active={step === 0 || undefined}
        >
          <p className="onboarding-step-intro">
            先选择你希望看到的界面方式，之后随时可以在设置中修改。
          </p>
          <div
            className="onboarding-mode-picker"
            role="group"
            aria-label="显示体验"
          >
            <button
              type="button"
              className="onboarding-mode-card"
              aria-pressed={experienceMode === "guided"}
              onClick={() => onExperienceModeChange("guided")}
            >
              <strong>引导体验</strong>
              <span>默认收起复杂选项，关键位置提供更多解释。</span>
            </button>
            <button
              type="button"
              className="onboarding-mode-card"
              aria-pressed={experienceMode === "complete"}
              onClick={() => onExperienceModeChange("complete")}
            >
              <strong>完整体验</strong>
              <span>默认显示更多控制项，适合希望快速调整细节的人。</span>
            </button>
          </div>
          <p className="onboarding-note">
            两种体验不减少任何能力，只改变默认显示方式；所有高级选项始终可以找到。
          </p>
          <div className="onboarding-demo-callout">
            <ShieldCheck size={17} aria-hidden="true" />
            <div>
              <strong>还没有 API Key？先体验演示模式</strong>
              <p>不调用模型、不读取真实项目，只执行一个安全的固定沙盒流程。</p>
              <Button
                variant="secondary"
                icon={<Play size={14} />}
                onClick={onOpenDemo}
              >
                开始沙盒演示
              </Button>
            </div>
          </div>
        </div>

        <div className="onboarding-list" data-active-step={step}>
          <OnboardingRow
            icon={<KeyRound size={15} aria-hidden="true" />}
            title="1. 检查 Claude Code"
            detail={
              claudeCliAvailable
                ? "已检测到 Claude Code，可以在本机启动会话。"
                : "CC Panel 需要调用本机的官方 Claude Code 命令行。"
            }
            ready={claudeCliAvailable}
            actions={
              <>
                {!claudeCliAvailable && (
                  <Button
                    variant="secondary"
                    busy={busy}
                    icon={<Clipboard size={14} />}
                    onClick={onCopyInstallCommand}
                  >
                    复制安装命令
                  </Button>
                )}
                <Button
                  variant="ghost"
                  busy={busy}
                  icon={<RotateCw size={14} />}
                  onClick={onRecheckClaude}
                >
                  重新检测
                </Button>
              </>
            }
          />
          <OnboardingRow
            icon={<FolderOpen size={15} aria-hidden="true" />}
            title="2. 选择工作文件夹"
            detail={
              projectLabel
                ? `当前项目：${projectLabel}`
                : "选择项目文件夹，Claude Code 会在其中工作。"
            }
            ready={Boolean(projectLabel)}
            actions={
              <Button
                variant="secondary"
                busy={busy}
                icon={<FolderOpen size={14} />}
                onClick={onSelectProject}
              >
                {projectLabel ? "更换文件夹" : "选择项目目录"}
              </Button>
            }
          />
          <OnboardingRow
            icon={<KeyRound size={15} aria-hidden="true" />}
            title="3. 选择默认模型"
            detail={
              modelReady
                ? "已配置可用模型，发送消息时会使用当前选中的模型。"
                : "添加一个模型配置；也可以稍后在模型栏完成。"
            }
            ready={modelReady}
            actions={
              <Button
                variant="secondary"
                busy={busy}
                icon={<KeyRound size={14} />}
                onClick={onAddModel}
              >
                {modelReady ? "管理模型" : "添加模型配置"}
              </Button>
            }
          />
          <OnboardingRow
            icon={<Sparkles size={15} aria-hidden="true" />}
            title="4. 本地 Prompt 优化"
            detail={
              ollama.online
                ? "Ollama 已连接。开启后，发送前可以用本地模型整理指令，原文不会被替换。"
                : "未连接 Ollama。可以跳过，之后安装或启动 Ollama 后再选择。"
            }
            ready={Boolean(ollama.online && ollama.selectedModel)}
            actions={
              <label className="onboarding-select">
                <span className="sr-only">选择本地 Prompt 优化模型</span>
                <select
                  aria-label="选择本地 Prompt 优化模型"
                  value={ollama.selectedModel ?? ""}
                  disabled={ollamaSaving || !ollama.models.length}
                  onChange={(event) =>
                    onSelectOllamaModel(event.target.value || null)
                  }
                >
                  <option value="">关闭本地优化</option>
                  {ollama.models.map((model) => (
                    <option key={model.name} value={model.name}>
                      {model.name}
                    </option>
                  ))}
                </select>
              </label>
            }
          />
          <OnboardingRow
            icon={<Play size={15} aria-hidden="true" />}
            title="5. 演示模式"
            detail="不调用真实模型，不读取真实项目，只在 CC Panel 沙盒目录创建一个欢迎文件。"
            ready={false}
            actions={
              <Button
                variant="secondary"
                icon={<Play size={14} />}
                onClick={onOpenDemo}
              >
                开始沙盒演示
              </Button>
            }
          />
        </div>

        <div className="onboarding-progress" aria-live="polite">
          <span>
            第 {step + 1} 步，共 {totalSteps} 步
          </span>
          <strong>{stepLabels[step]}</strong>
        </div>
        <footer className="model-dialog__actions onboarding-dialog__footer">
          <button
            type="button"
            className="button button--ghost"
            disabled={busy}
            onClick={onClose}
          >
            跳过全部
          </button>
          <div className="onboarding-dialog__step-actions">
            {step > 0 && (
              <button
                type="button"
                className="button button--ghost"
                disabled={busy}
                onClick={() => setStep((value) => value - 1)}
              >
                <ChevronLeft size={14} aria-hidden="true" />
                上一步
              </button>
            )}
            <button
              type="button"
              className="button button--ghost"
              disabled={busy}
              onClick={() => {
                if (step >= totalSteps - 1) onClose();
                else setStep((value) => value + 1);
              }}
            >
              {step >= totalSteps - 1 ? "稍后完成" : "跳过这一步"}
            </button>
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => {
                if (step >= totalSteps - 1) onClose();
                else setStep((value) => value + 1);
              }}
            >
              {step >= totalSteps - 1 ? "进入 CC Panel" : "下一步"}
              {step < totalSteps - 1 && (
                <ChevronRight size={14} aria-hidden="true" />
              )}
            </Button>
          </div>
        </footer>
      </section>
    </div>
  );
}

function OnboardingRow({
  icon,
  title,
  detail,
  ready,
  actions,
}: {
  icon: ReactNode;
  title: string;
  detail: string;
  ready: boolean;
  actions: ReactNode;
}) {
  return (
    <div className="onboarding-item" data-ready={ready || undefined}>
      <span className="onboarding-item__badge" aria-hidden="true">
        {ready ? <Check size={15} /> : icon}
      </span>
      <div className="onboarding-item__body">
        <strong>{title}</strong>
        <p>{detail}</p>
        <div className="onboarding-item__actions">{actions}</div>
      </div>
    </div>
  );
}
