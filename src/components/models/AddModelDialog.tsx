import { cloneElement, useEffect, useId, useRef, useState } from "react";
import type { ReactElement } from "react";
import { KeyRound, X } from "lucide-react";
import type { ModelProfile, ModelProfileInput } from "../../api/dto";
import {
  normalizeAnthropicBaseUrl,
  presetFor,
  PROVIDER_GROUPS,
} from "./providerCatalog";

interface Props {
  profile?: ModelProfile | null;
  busy?: boolean;
  onClose: () => void;
  onSave: (profile: ModelProfileInput, promptForApiKey: boolean) => void;
}

interface FormErrors {
  providerName?: string;
  baseUrl?: string;
  modelId?: string;
}

const focusableSelector =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function AddModelDialog({
  profile = null,
  busy = false,
  onClose,
  onSave,
}: Props) {
  const panelRef = useRef<HTMLElement>(null);
  const firstInputRef = useRef<HTMLSelectElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const closeRef = useRef(onClose);
  const busyRef = useRef(busy);
  const titleId = useId();
  const descriptionId = useId();
  const [providerName, setProviderName] = useState(
    profile?.providerName ?? "Claude Official",
  );
  const [note, setNote] = useState(profile?.note ?? "");
  const [websiteUrl, setWebsiteUrl] = useState(profile?.websiteUrl ?? "");
  const [baseUrl, setBaseUrl] = useState(
    profile?.baseUrl ?? "https://api.anthropic.com",
  );
  const [modelId, setModelId] = useState(profile?.modelId ?? "claude-opus-5");
  const [replaceApiKey, setReplaceApiKey] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});
  const preset = presetFor(providerName);

  closeRef.current = onClose;
  busyRef.current = busy;

  useEffect(() => {
    previousFocus.current = document.activeElement as HTMLElement;
    const frame = document.querySelector<HTMLElement>(".app-frame");
    const backdrop = panelRef.current?.parentElement;
    const background = Array.from(frame?.children ?? []).filter(
      (element): element is HTMLElement =>
        element instanceof HTMLElement && element !== backdrop,
    );
    background.forEach((element) => {
      element.inert = true;
    });
    firstInputRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      const panel = panelRef.current;
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
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !panel.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (
        !event.shiftKey &&
        (active === last || !panel.contains(active))
      ) {
        event.preventDefault();
        first.focus();
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
  }, []);

  const validate = () => {
    const next: FormErrors = {};
    if (!providerName.trim()) next.providerName = "请输入提供商名称。";
    if (!baseUrl.trim()) next.baseUrl = "请输入 API 地址。";
    else if (!isHttpUrl(baseUrl)) next.baseUrl = "请输入有效的 http(s) 地址。";
    if (!modelId.trim()) next.modelId = "请输入模型 ID。";
    setErrors(next);
    return Object.keys(next).length === 0;
  };

  const save = () => {
    if (!validate()) return;
    const normalizedBaseUrl = normalizeAnthropicBaseUrl(baseUrl);
    if (normalizedBaseUrl.normalized) setBaseUrl(normalizedBaseUrl.url);
    const promptForApiKey = !profile || replaceApiKey;
    onSave(
      {
        id: profile?.id,
        providerName: providerName.trim(),
        note: note.trim() || null,
        websiteUrl: websiteUrl.trim() || null,
        baseUrl: normalizedBaseUrl.url,
        modelId: modelId.trim(),
        selected: profile?.selected ?? false,
      },
      promptForApiKey,
    );
  };

  return (
    <div className="modal-backdrop">
      <section
        ref={panelRef}
        className="model-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <header className="model-dialog__header">
          <div className="model-dialog__title">
            <span className="model-dialog__icon" aria-hidden="true">
              <KeyRound size={17} />
            </span>
            <div>
              <h2 id={titleId}>{profile ? "编辑模型配置" : "添加模型配置"}</h2>
              <p id={descriptionId}>连接信息保存在本地，API 密钥不会回显。</p>
            </div>
          </div>
          <button
            type="button"
            className="header-icon-button"
            aria-label="关闭"
            disabled={busy}
            onClick={onClose}
          >
            <X size={17} aria-hidden="true" />
          </button>
        </header>
        <form
          className="model-dialog__form"
          onSubmit={(event) => {
            event.preventDefault();
            save();
          }}
          noValidate
        >
          <div className="form-grid form-grid--two">
            <Field label="提供商" error={errors.providerName}>
              <select
                ref={firstInputRef}
                value={providerName}
                onChange={(event) => {
                  const next = event.target.value;
                  setProviderName(next);
                  const preset = presetFor(next);
                  if (preset) {
                    if (preset.baseUrl) setBaseUrl(preset.baseUrl);
                    if (preset.modelId) setModelId(preset.modelId);
                  }
                }}
                aria-invalid={Boolean(errors.providerName)}
              >
                <option value="" disabled>
                  {preset
                    ? "选择供应商…"
                    : providerName
                      ? `当前：${providerName}（不在预设中）`
                      : "选择供应商…"}
                </option>
                {PROVIDER_GROUPS.map((group) => (
                  <optgroup key={group.group} label={group.group}>
                    {group.items.map((item) => (
                      <option key={item.value} value={item.value}>
                        {item.label}
                      </option>
                    ))}
                  </optgroup>
                ))}
                <option value="自定义">自定义（Anthropic 兼容）</option>
              </select>
            </Field>
            <Field
              label="模型 ID"
              error={errors.modelId}
              help={
                providerName === "DeepSeek"
                  ? "DeepSeek 官方 Anthropic 兼容模型：deepseek-v4-flash（可加 [1M] 后缀启用 1M 上下文）。"
                  : preset
                    ? `默认 ${preset.modelId}，可按账号权限修改。${preset.note ? ` ${preset.note}` : ""}`
                    : undefined
              }
            >
              <input
                value={modelId}
                onChange={(event) => setModelId(event.target.value)}
                aria-invalid={Boolean(errors.modelId)}
                placeholder="claude-opus-5"
              />
            </Field>
          </div>
          <Field
            label="API 地址"
            error={errors.baseUrl}
            help={
              providerName === "DeepSeek"
                ? "DeepSeek 的 Anthropic 兼容端点为 https://api.deepseek.com/anthropic，勿填 platform.deepseek.com（那是网页平台）。"
                : preset
                  ? preset.baseUrl
                    ? `已填入 Anthropic 兼容端点 ${preset.baseUrl}；如报错请对照官方文档。${preset.note ? ` ${preset.note}` : ""}`
                    : `该供应商未收录现成端点，请填写 API 地址。${preset.note ? ` ${preset.note}` : ""}`
                  : "仅允许 http(s) 地址；不要把密钥写在 URL 中。"
            }
          >
            <input
              type="url"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              aria-invalid={Boolean(errors.baseUrl)}
              placeholder="https://api.anthropic.com"
            />
          </Field>
          {profile?.hasApiKey && (
            <label className="form-field form-field--checkbox">
              <input
                type="checkbox"
                checked={replaceApiKey}
                onChange={(event) => setReplaceApiKey(event.target.checked)}
              />
              <span>
                <span className="form-field__label">替换 API 密钥</span>
                <small>
                  保存时打开 Windows 系统凭据窗口；不勾选则保留现有密钥。
                </small>
              </span>
            </label>
          )}
          {!profile && (
            <p className="model-dialog__credential-note">
              保存时将打开 Windows 系统凭据窗口输入 API 密钥。密钥不会进入 React
              或普通 IPC。
            </p>
          )}
          <div className="form-grid form-grid--two">
            <Field label="备注">
              <input
                value={note}
                onChange={(event) => setNote(event.target.value)}
                placeholder="例如：个人 API"
              />
            </Field>
            <Field label="官网（可选）">
              <input
                type="url"
                value={websiteUrl}
                onChange={(event) => setWebsiteUrl(event.target.value)}
                placeholder="https://…"
              />
            </Field>
          </div>
          <footer className="model-dialog__actions">
            <button
              type="button"
              className="button button--ghost"
              disabled={busy}
              onClick={onClose}
            >
              取消
            </button>
            <button
              type="submit"
              className="button button--primary"
              disabled={busy}
            >
              {busy ? "保存中…" : "保存配置"}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}

function Field({
  label,
  error,
  help,
  children,
}: {
  label: string;
  error?: string;
  help?: string;
  children: ReactElement<{ id?: string; "aria-describedby"?: string }>;
}) {
  const id = useId();
  const errorId = `${id}-error`;
  const helpId = `${id}-help`;
  return (
    <label className="form-field" htmlFor={id}>
      <span className="form-field__label">{label}</span>
      {cloneElement(children, {
        id,
        "aria-describedby":
          [help && helpId, error && errorId].filter(Boolean).join(" ") ||
          undefined,
      })}
      {help && <small id={helpId}>{help}</small>}
      {error && (
        <span id={errorId} className="form-field__error" role="alert">
          {error}
        </span>
      )}
    </label>
  );
}

function isHttpUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}
