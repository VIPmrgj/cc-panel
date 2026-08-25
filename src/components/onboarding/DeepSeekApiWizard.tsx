import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  ExternalLink,
  KeyRound,
  ShieldCheck,
  X,
} from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type {
  ModelConnectionTestResult,
  ModelProfile,
  ModelProfileInput,
} from "../../api/dto";
import { Button } from "../common/Button";
import { presetFor } from "../models/providerCatalog";

interface Props {
  open: boolean;
  saving?: boolean;
  testing?: boolean;
  savedProfile?: ModelProfile | null;
  testResult?: ModelConnectionTestResult | null;
  onSave: (profile: ModelProfileInput) => void;
  onTest: (profileId: string) => void;
  onOpenAdvanced: (profile?: ModelProfile | null) => void;
  onClose: () => void;
}

const focusableSelector =
  'button:not([disabled]), input:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

export function DeepSeekApiWizard({
  open,
  saving = false,
  testing = false,
  savedProfile = null,
  testResult = null,
  onSave,
  onTest,
  onOpenAdvanced,
  onClose,
}: Props) {
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const closeRef = useRef(onClose);
  const [step, setStep] = useState(0);
  const [hasCopiedKey, setHasCopiedKey] = useState(false);
  const [costConfirmed, setCostConfirmed] = useState(false);
  const preset = presetFor("DeepSeek");

  closeRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    setStep(savedProfile ? 3 : 0);
    setHasCopiedKey(false);
    setCostConfirmed(false);
    previousFocus.current = document.activeElement as HTMLElement;
    const panel = panelRef.current;
    const frame = document.querySelector<HTMLElement>(".app-frame");
    const backdrop = panel?.parentElement;
    const background = Array.from(frame?.children ?? []).filter(
      (element): element is HTMLElement =>
        element instanceof HTMLElement && element !== backdrop,
    );
    background.forEach((element) => {
      element.inert = true;
    });
    const first = panel?.querySelector<HTMLElement>(focusableSelector);
    first?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (!panel) return;
      if (event.key === "Escape" && !saving && !testing) {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(
        panel.querySelectorAll<HTMLElement>(focusableSelector),
      );
      if (!focusable.length) return;
      const firstFocusable = focusable[0];
      const lastFocusable = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === firstFocusable) {
        event.preventDefault();
        lastFocusable.focus();
      } else if (!event.shiftKey && document.activeElement === lastFocusable) {
        event.preventDefault();
        firstFocusable.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      background.forEach((element) => {
        element.inert = false;
      });
      previousFocus.current?.focus();
    };
  }, [open, savedProfile, saving, testing]);

  if (!open) return null;

  if (!preset) {
    return (
      <div className="modal-backdrop">
        <section
          className="model-dialog deepseek-wizard"
          role="dialog"
          aria-modal="true"
        >
          <div className="deepseek-wizard__body">
            <p className="deepseek-wizard__error">DeepSeek 预设暂时不可用。</p>
            <Button variant="ghost" onClick={onClose}>
              关闭
            </Button>
          </div>
        </section>
      </div>
    );
  }

  const saveProfile: ModelProfileInput = {
    providerName: preset.value,
    note: "通过 DeepSeek 新手引导配置",
    websiteUrl: "https://platform.deepseek.com/",
    baseUrl: preset.baseUrl,
    modelId: preset.modelId,
    selected: true,
  };

  const canTest = Boolean(savedProfile && costConfirmed && !testing);
  const progressLabel = "第 " + (step + 1) + " 步，共 4 步";

  return (
    <div className="modal-backdrop">
      <section
        ref={panelRef}
        className="model-dialog deepseek-wizard"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <header className="model-dialog__header">
          <div className="model-dialog__title">
            <span className="model-dialog__icon" aria-hidden="true">
              <KeyRound size={18} />
            </span>
            <div>
              <h2 id={titleId}>用 DeepSeek 配置模型</h2>
              <p id={descriptionId}>
                这是一个示例流程，其他 API 仍可在高级配置中使用。
              </p>
            </div>
          </div>
          <button
            type="button"
            className="header-icon-button"
            aria-label="关闭 DeepSeek 引导"
            disabled={saving || testing}
            onClick={onClose}
          >
            <X size={17} aria-hidden="true" />
          </button>
        </header>

        <div className="deepseek-wizard__progress" aria-live="polite">
          <span>{progressLabel}</span>
          <strong>
            {["了解流程", "申请 API Key", "安全保存", "测试连接"][step]}
          </strong>
        </div>

        <div className="deepseek-wizard__body">
          {step === 0 && (
            <div className="deepseek-wizard__step">
              <div className="deepseek-wizard__hero">
                <span className="deepseek-wizard__hero-icon" aria-hidden="true">
                  <KeyRound size={25} />
                </span>
                <div>
                  <h3>先用一个例子学会接入模型</h3>
                  <p>
                    API Key 可以理解成模型服务的密码。CC Panel
                    不会读取你的账号密码，只会指导你创建密钥并安全保存。
                  </p>
                </div>
              </div>
              <div className="deepseek-wizard__info-grid">
                <div>
                  <strong>你需要准备</strong>
                  <span>一个 DeepSeek 账号和一个 API Key。</span>
                </div>
                <div>
                  <strong>费用说明</strong>
                  <span>注册、充值和调用费用以服务商页面显示为准。</span>
                </div>
                <div>
                  <strong>安全说明</strong>
                  <span>密钥通过系统凭据窗口保存，界面不会回显完整密钥。</span>
                </div>
              </div>
              <div className="deepseek-wizard__actions-card">
                <p>如果你已经有其他服务商的 API Key，可以直接使用通用配置。</p>
                <Button
                  variant="ghost"
                  onClick={() => onOpenAdvanced(null)}
                  icon={<ArrowRight size={14} />}
                >
                  我已有其他 API Key
                </Button>
              </div>
            </div>
          )}

          {step === 1 && (
            <div className="deepseek-wizard__step">
              <div className="deepseek-wizard__step-heading">
                <span className="deepseek-wizard__step-number">1</span>
                <div>
                  <h3>申请 DeepSeek API Key</h3>
                  <p>
                    你将在浏览器中完成注册和创建密钥，CC Panel
                    不会读取网页内容。
                  </p>
                </div>
              </div>
              <ol className="deepseek-wizard__checklist">
                <li>打开 DeepSeek 开放平台并注册或登录。</li>
                <li>进入 API Key / 密钥管理页面。</li>
                <li>创建一个新的 API Key 并复制它。</li>
                <li>回到这里，点击下一步保存密钥。</li>
              </ol>
              <a
                className="deepseek-wizard__external-link"
                href="https://platform.deepseek.com/"
                target="_blank"
                rel="noreferrer"
              >
                <ExternalLink size={15} aria-hidden="true" />
                打开 DeepSeek 开放平台
              </a>
              <label className="deepseek-wizard__checkbox">
                <input
                  type="checkbox"
                  checked={hasCopiedKey}
                  onChange={(event) => setHasCopiedKey(event.target.checked)}
                />
                <span>我已经创建并复制了 API Key</span>
              </label>
            </div>
          )}

          {step === 2 && (
            <div className="deepseek-wizard__step">
              <div className="deepseek-wizard__step-heading">
                <span className="deepseek-wizard__step-number">2</span>
                <div>
                  <h3>保存密钥并使用推荐配置</h3>
                  <p>
                    点击保存后，系统会弹出安全凭据窗口，请在那里输入 API Key。
                  </p>
                </div>
              </div>
              <div className="deepseek-wizard__config-preview">
                <div>
                  <span>服务商</span>
                  <strong>{preset.label}</strong>
                </div>
                <div>
                  <span>API 地址</span>
                  <code>{preset.baseUrl}</code>
                </div>
                <div>
                  <span>推荐模型</span>
                  <code>{preset.modelId}</code>
                </div>
              </div>
              <div className="deepseek-wizard__security-note">
                <ShieldCheck size={18} aria-hidden="true" />
                <p>
                  API Key 不会进入聊天内容、React 状态或普通 IPC
                  请求。保存后只显示“密钥已保存”。
                </p>
              </div>
              <Button
                variant="ghost"
                onClick={() => onOpenAdvanced(null)}
                disabled={saving}
              >
                打开高级配置
              </Button>
            </div>
          )}

          {step === 3 && (
            <div className="deepseek-wizard__step">
              {testResult?.ok ? (
                <div className="deepseek-wizard__result deepseek-wizard__result--success">
                  <CheckCircle2 size={28} aria-hidden="true" />
                  <div>
                    <h3>连接成功</h3>
                    <p>{testResult.message}</p>
                    <small>
                      现在可以返回新手引导，继续选择工作目录和体验演示。
                    </small>
                  </div>
                </div>
              ) : (
                <>
                  <div className="deepseek-wizard__step-heading">
                    <span className="deepseek-wizard__step-number">3</span>
                    <div>
                      <h3>测试连接</h3>
                      <p>
                        配置已经保存，但还没有调用模型。测试会发送一个最小请求。
                      </p>
                    </div>
                  </div>
                  {testResult && (
                    <div className="deepseek-wizard__result deepseek-wizard__result--error">
                      <AlertTriangle size={22} aria-hidden="true" />
                      <div>
                        <h3>连接没有成功</h3>
                        <p>{testResult.message}</p>
                        <small>错误代码：{testResult.code}</small>
                      </div>
                    </div>
                  )}
                  <label className="deepseek-wizard__checkbox">
                    <input
                      type="checkbox"
                      checked={costConfirmed}
                      onChange={(event) =>
                        setCostConfirmed(event.target.checked)
                      }
                    />
                    <span>我知道测试可能产生少量 API 费用</span>
                  </label>
                  <div className="deepseek-wizard__actions-card">
                    <Button
                      variant="ghost"
                      onClick={() => onOpenAdvanced(savedProfile)}
                    >
                      修改配置或重新输入密钥
                    </Button>
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        <footer className="model-dialog__actions deepseek-wizard__footer">
          <Button
            variant="ghost"
            disabled={saving || testing}
            onClick={step === 0 ? onClose : () => setStep((value) => value - 1)}
            icon={step > 0 ? <ArrowLeft size={14} /> : undefined}
          >
            {step === 0 ? "暂时跳过" : "上一步"}
          </Button>
          <div className="deepseek-wizard__footer-actions">
            {step === 0 && (
              <Button
                variant="primary"
                onClick={() => setStep(1)}
                icon={<ArrowRight size={14} />}
              >
                开始示例
              </Button>
            )}
            {step === 1 && (
              <Button
                variant="primary"
                disabled={!hasCopiedKey}
                onClick={() => setStep(2)}
                icon={<ArrowRight size={14} />}
              >
                下一步
              </Button>
            )}
            {step === 2 && (
              <Button
                variant="primary"
                busy={saving}
                onClick={() => onSave(saveProfile)}
                icon={<KeyRound size={14} />}
              >
                保存并输入 API Key
              </Button>
            )}
            {step === 3 && !testResult?.ok && (
              <Button
                variant="primary"
                busy={testing}
                disabled={!canTest}
                onClick={() => {
                  if (savedProfile) onTest(savedProfile.id);
                }}
              >
                测试连接
              </Button>
            )}
            {step === 3 && testResult?.ok && (
              <Button
                variant="primary"
                onClick={onClose}
                icon={<CheckCircle2 size={14} />}
              >
                返回新手引导
              </Button>
            )}
          </div>
        </footer>
      </section>
    </div>
  );
}
