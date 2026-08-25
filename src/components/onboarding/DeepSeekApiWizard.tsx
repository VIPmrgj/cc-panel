import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  Copy,
  KeyRound,
  X,
} from "lucide-react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useEffect, useId, useRef, useState } from "react";
import type {
  ModelConnectionTestResult,
  ModelProfile,
  ModelProfileInput,
} from "../../api/dto";
import { Button } from "../common/Button";
import { presetFor } from "../models/providerCatalog";
import loginImage from "../../assets/onboarding/deepseek/登录界面.png";
import homeImage from "../../assets/onboarding/deepseek/主页.png";
import rechargeImage from "../../assets/onboarding/deepseek/去充值.png";
import apiKeyImage from "../../assets/onboarding/deepseek/api key.png";
import apiNameImage from "../../assets/onboarding/deepseek/api名称.png";
import copyApiKeyImage from "../../assets/onboarding/deepseek/复制api key.png";

interface Props {
  open: boolean;
  saving?: boolean;
  testing?: boolean;
  savedProfile?: ModelProfile | null;
  testResult?: ModelConnectionTestResult | null;
  onSave: (
    profile: ModelProfileInput,
    apiKey: string,
  ) => void | Promise<boolean | void>;
  onTest: (profileId: string) => void;
  onOpenAdvanced: (profile?: ModelProfile | null) => void;
  onClose: () => void;
}

const focusableSelector =
  'button:not([disabled]), input:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

type TutorialImage = {
  src: string;
  alt: string;
  title: string;
};

function TutorialImageCard({
  image,
  onOpen,
}: {
  image: TutorialImage;
  onOpen: (image: TutorialImage) => void;
}) {
  return (
    <figure className="deepseek-tutorial-image">
      <button
        type="button"
        className="deepseek-tutorial-image__button"
        onClick={() => onOpen(image)}
        onDoubleClick={() => onOpen(image)}
        aria-label={`放大查看：${image.title}`}
      >
        <img src={image.src} alt={image.alt} />
      </button>
      <figcaption>{image.title}（点击或双击放大）</figcaption>
    </figure>
  );
}

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
  const panelRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const closeRef = useRef(onClose);
  const busyRef = useRef(false);
  const [step, setStep] = useState(0);
  const [apiKey, setApiKey] = useState("");
  const [apiKeyError, setApiKeyError] = useState("");
  const [previewImage, setPreviewImage] = useState<TutorialImage | null>(null);
  const [urlCopied, setUrlCopied] = useState(false);
  const [urlCopyError, setUrlCopyError] = useState("");
  const [costConfirmed, setCostConfirmed] = useState(false);
  const preset = presetFor("DeepSeek");

  closeRef.current = onClose;
  busyRef.current = saving || testing;

  useEffect(() => {
    setUrlCopied(false);
    setUrlCopyError("");
    if (!open) return;
    setStep(savedProfile ? 2 : 0);
    setApiKey("");
    setApiKeyError("");
    setPreviewImage(null);
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
      if (event.key === "Escape" && !busyRef.current) {
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
  }, [open, savedProfile]);

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
    providerName: "默认模型",
    note: "DeepSeek",
    websiteUrl: "https://platform.deepseek.com/",
    baseUrl: preset.baseUrl,
    modelId: preset.modelId,
    selected: true,
  };

  const canTest = Boolean(savedProfile && costConfirmed && !testing);
  const pageTitle =
    step === 0 ? "认识 API" : step === 1 ? "三步拿到“加油卡”" : "测试连接";
  const submitApiKey = async () => {
    const normalizedKey = apiKey.trim();
    if (!normalizedKey) {
      setApiKeyError("请先粘贴 DeepSeek API Key。");
      return;
    }
    if (!normalizedKey.startsWith("sk-")) {
      setApiKeyError("API Key 通常以 sk- 开头，请检查是否复制完整。");
      return;
    }
    setApiKeyError("");
    try {
      const result = await onSave(saveProfile, normalizedKey);
      if (result !== false) {
        setApiKey("");
        setStep(2);
      }
    } catch {
      setApiKeyError("保存失败，请检查网络或模型配置后重试。");
    }
  };
  const copyPlatformUrl = async () => {
    try {
      await writeText("https://platform.deepseek.com/");
      setUrlCopied(true);
      setUrlCopyError("");
    } catch {
      setUrlCopyError("网址复制失败，请手动复制。");
    }
  };

  return (
    <div className="modal-backdrop">
      <section
        ref={panelRef}
        className="model-dialog deepseek-wizard"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <header className="model-dialog__header">
          <div className="model-dialog__title">
            <span className="model-dialog__icon" aria-hidden="true">
              <KeyRound size={18} />
            </span>
            <div>
              <h2 id={titleId}>{pageTitle}</h2>
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

        <div className="deepseek-wizard__body">
          {step === 0 && (
            <div className="deepseek-wizard__step">
              <div className="deepseek-wizard__hero">
                <div>
                  <h3>为什么需要 API Key？——就像给汽车加油</h3>
                  <p>
                    本软件就像一辆好用的汽车，但车要跑起来，得有汽油。AI
                    模型就是“汽油”，而{" "}
                    <strong>API Key 就是你自己的加油卡</strong>。
                  </p>
                  <p>
                    软件不卖油，也不赚差价。你需要自己去 DeepSeek
                    官方办一张“加油卡”，充点钱，然后插到软件里，车就能跑了。钱直接给
                    DeepSeek，用多少扣多少，账单自己随时能查。不想用了，把卡删掉就行，余额还是你的。
                  </p>
                </div>
              </div>
              <div className="deepseek-wizard__info-grid">
                <div>
                  <strong>你要准备什么</strong>
                  <span>一个 DeepSeek 账号，后面创建一枚 API Key。</span>
                </div>
                <div>
                  <strong>会产生费用吗</strong>
                  <span>后续充值和模型调用费用以 DeepSeek 页面显示为准。</span>
                </div>
                <div>
                  <strong>密钥安全吗</strong>
                  <span>
                    在本软件的密码框粘贴，保存后只保留受保护的本机凭据，界面不会回显完整密钥。
                  </span>
                </div>
              </div>
            </div>
          )}

          {step === 1 && (
            <div className="deepseek-wizard__step deepseek-tutorial">
              <div className="deepseek-tutorial__intro">
                按下面的顺序完成操作。图片可以点击放大，API Key
                只显示一次，请复制完整。
              </div>

              <section className="deepseek-tutorial__section">
                <div className="deepseek-tutorial__section-heading">
                  <span>1</span>
                  <div>
                    <strong>打开 platform.deepseek.com，注册并登录。</strong>
                    <button
                      type="button"
                      className="deepseek-tutorial__copy-box"
                      onClick={() => void copyPlatformUrl()}
                      aria-label="复制网址 https://platform.deepseek.com/"
                    >
                      <code>https://platform.deepseek.com/</code>
                      <span>{urlCopied ? "已复制" : "点击复制网址"}</span>
                      <Copy size={16} aria-hidden="true" />
                    </button>
                    {urlCopyError && (
                      <small className="deepseek-tutorial__copy-error">
                        {urlCopyError}
                      </small>
                    )}
                  </div>
                </div>
                <TutorialImageCard
                  image={{
                    src: loginImage,
                    alt: "DeepSeek 登录界面示意图",
                    title: "登录界面",
                  }}
                  onOpen={setPreviewImage}
                />
              </section>

              <section className="deepseek-tutorial__section">
                <div className="deepseek-tutorial__section-heading">
                  <span>2</span>
                  <div>
                    <strong>
                      点击左侧 “API Keys” → “创建”，复制那串 sk-
                      开头的密钥（只显示一次，务必保存）。
                    </strong>
                    <p>如果提示余额不足，去 DeepSeek 充几块钱即可。</p>
                  </div>
                </div>
                <div className="deepseek-tutorial__images">
                  <TutorialImageCard
                    image={{
                      src: homeImage,
                      alt: "DeepSeek 登录后的主页",
                      title: "登录后的主页",
                    }}
                    onOpen={setPreviewImage}
                  />
                  <TutorialImageCard
                    image={{
                      src: rechargeImage,
                      alt: "DeepSeek 充值页面",
                      title: "余额不足时前往充值",
                    }}
                    onOpen={setPreviewImage}
                  />
                  <TutorialImageCard
                    image={{
                      src: apiKeyImage,
                      alt: "DeepSeek API Keys 页面",
                      title: "打开 API Keys",
                    }}
                    onOpen={setPreviewImage}
                  />
                  <TutorialImageCard
                    image={{
                      src: apiNameImage,
                      alt: "创建 API Key 时填写名称",
                      title: "创建 API Key",
                    }}
                    onOpen={setPreviewImage}
                  />
                  <TutorialImageCard
                    image={{
                      src: copyApiKeyImage,
                      alt: "复制 DeepSeek API Key",
                      title: "复制 sk- 开头的密钥",
                    }}
                    onOpen={setPreviewImage}
                  />
                </div>
              </section>

              <section className="deepseek-tutorial__section">
                <div className="deepseek-tutorial__section-heading">
                  <span>3</span>
                  <div>
                    <strong>回到本软件，粘贴密钥，保存。</strong>
                    <p>
                      保存后会自动创建并选中一个名为“默认模型”的 DeepSeek 配置。
                    </p>
                  </div>
                </div>
                <div className="deepseek-tutorial__key-card">
                  <label htmlFor="deepseek-api-key">
                    在这里粘贴 DeepSeek API Key
                  </label>
                  <input
                    id="deepseek-api-key"
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    value={apiKey}
                    placeholder="sk-..."
                    aria-invalid={Boolean(apiKeyError)}
                    onChange={(event) => {
                      setApiKey(event.target.value);
                      setApiKeyError("");
                    }}
                  />
                  <span>
                    密钥通常以 sk- 开头，只显示一次，请确认已经复制完整。
                  </span>
                  {apiKeyError && (
                    <small className="deepseek-tutorial__key-error">
                      {apiKeyError}
                    </small>
                  )}
                </div>
              </section>
            </div>
          )}

          {step === 2 && (
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
                  <div className="deepseek-test-intro">
                    <strong>配置已经保存</strong>
                    <p>
                      点击下面的按钮，发送一条很短的请求确认 API Key 可以使用。
                    </p>
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
                  <div className="deepseek-test-consent">
                    <label className="deepseek-test-consent__label">
                      <input
                        type="checkbox"
                        checked={costConfirmed}
                        onChange={(event) =>
                          setCostConfirmed(event.target.checked)
                        }
                      />
                      <span>
                        <strong>我知道测试可能产生少量 API 费用</strong>
                        <small>
                          测试只发送一条很短的请求，不会执行项目文件操作。
                        </small>
                      </span>
                    </label>
                  </div>
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
                开始配置 DeepSeek
              </Button>
            )}
            {step === 1 && (
              <Button
                variant="primary"
                busy={saving}
                disabled={!apiKey.trim() || saving}
                onClick={submitApiKey}
                icon={<KeyRound size={14} />}
              >
                保存并配置默认模型
              </Button>
            )}
            {step === 2 && !testResult?.ok && (
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
            {step === 2 && testResult?.ok && (
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
      {previewImage && (
        <div
          className="deepseek-image-lightbox"
          role="dialog"
          aria-modal="true"
          aria-label={`放大查看：${previewImage.title}`}
          onClick={() => setPreviewImage(null)}
        >
          <div
            className="deepseek-image-lightbox__panel"
            onClick={(event) => event.stopPropagation()}
          >
            <button
              type="button"
              className="deepseek-image-lightbox__close"
              aria-label="关闭图片预览"
              onClick={() => setPreviewImage(null)}
            >
              <X size={18} aria-hidden="true" />
            </button>
            <img src={previewImage.src} alt={previewImage.alt} />
            <p>{previewImage.title}</p>
          </div>
        </div>
      )}
    </div>
  );
}
