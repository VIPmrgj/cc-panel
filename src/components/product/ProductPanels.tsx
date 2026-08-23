import { useEffect, useState } from "react";
import {
  CheckCircle2,
  Download,
  ExternalLink,
  FolderOpen,
  RefreshCw,
  Save,
  ShieldAlert,
  Wrench,
} from "lucide-react";
import type {
  DiagnosticResult,
  EnvironmentCheck,
  EnvironmentReport,
  ProjectMemory,
  ProjectMemoryInput,
  UpdateInfo,
} from "../../api/dto";

export function ProjectMemoryPanel({
  memory,
  saving,
  onSave,
}: {
  memory: ProjectMemory | null | undefined;
  saving: boolean;
  onSave: (input: ProjectMemoryInput) => void;
}) {
  const [draft, setDraft] = useState<ProjectMemoryInput>(emptyMemory);
  useEffect(() => {
    if (memory)
      setDraft({
        enabled: memory.enabled,
        purpose: memory.purpose,
        techStack: memory.techStack,
        rules: memory.rules,
        avoid: memory.avoid,
        testCommand: memory.testCommand,
        preferredLanguage: memory.preferredLanguage,
      });
  }, [memory]);
  const update = (key: keyof ProjectMemoryInput, value: string | boolean) =>
    setDraft((current) => ({ ...current, [key]: value }));
  return (
    <section className="product-card" aria-labelledby="project-memory-title">
      <div className="product-card__header">
        <div>
          <p className="panel-eyebrow">PROJECT MEMORY</p>
          <h3 id="project-memory-title">项目记忆</h3>
          <p>把项目长期规则交给 Agent，每次发送时自动带上。</p>
        </div>
        <label className="compact-toggle">
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(e) => update("enabled", e.target.checked)}
          />
          <span>启用</span>
        </label>
      </div>
      <div className="product-form-grid">
        <Field
          label="项目目标"
          value={draft.purpose}
          onChange={(v) => update("purpose", v)}
          placeholder="例如：这是一个面向小白用户的桌面 Agent 客户端。"
          area
        />
        <Field
          label="技术栈"
          value={draft.techStack}
          onChange={(v) => update("techStack", v)}
          placeholder="例如：React + Tauri + Rust"
        />
        <Field
          label="必须遵守"
          value={draft.rules}
          onChange={(v) => update("rules", v)}
          placeholder="例如：中文回复；先检查再修改。"
          area
        />
        <Field
          label="避免事项"
          value={draft.avoid}
          onChange={(v) => update("avoid", v)}
          placeholder="例如：不要引入重量级依赖。"
          area
        />
        <Field
          label="测试命令"
          value={draft.testCommand}
          onChange={(v) => update("testCommand", v)}
          placeholder="例如：npm run test:run"
        />
        <Field
          label="偏好语言"
          value={draft.preferredLanguage}
          onChange={(v) => update("preferredLanguage", v)}
          placeholder="例如：中文"
        />
      </div>
      <div className="product-card__footer">
        <small>只保存到本机 CC Panel 数据目录，不会写入项目文件。</small>
        <button
          type="button"
          className="button button--secondary"
          disabled={saving || !memory}
          onClick={() => onSave(draft)}
        >
          <Save size={14} aria-hidden="true" />
          {saving ? "保存中…" : "保存项目记忆"}
        </button>
      </div>
    </section>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
  area = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  area?: boolean;
}) {
  return (
    <label>
      <span>{label}</span>
      {area ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
        />
      ) : (
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
        />
      )}
    </label>
  );
}

export function EnvironmentPanel({
  report,
  loading,
  repairId,
  onRefresh,
  onRepair,
  onChooseProject,
  onOpenModels,
}: {
  report?: EnvironmentReport;
  loading: boolean;
  repairId: string | null;
  onRefresh: () => void;
  onRepair: (check: EnvironmentCheck) => void;
  onChooseProject: () => void;
  onOpenModels: () => void;
}) {
  return (
    <section className="product-card" aria-labelledby="environment-title">
      <div className="product-card__header">
        <div>
          <p className="panel-eyebrow">ENVIRONMENT</p>
          <h3 id="environment-title">环境自检</h3>
          <p>检查 Claude、项目目录、模型、Git、Ollama 和 Skills。</p>
        </div>
        <button
          type="button"
          className="panel-icon-button"
          onClick={onRefresh}
          disabled={loading}
          title="重新检查"
        >
          <RefreshCw
            size={15}
            className={loading ? "spin" : undefined}
            aria-hidden="true"
          />
        </button>
      </div>
      {!report ? (
        <div className="product-empty">
          <p>还没有检查结果。</p>
          <button
            type="button"
            className="button button--secondary"
            onClick={onRefresh}
          >
            开始检查
          </button>
        </div>
      ) : (
        <div className="environment-list" aria-live="polite">
          {report.checks.map((check) => (
            <div
              className="environment-item"
              data-status={check.status}
              key={check.id}
            >
              <span className="environment-item__icon" aria-hidden="true">
                {check.status === "ok" ? (
                  <CheckCircle2 size={17} />
                ) : (
                  <ShieldAlert size={17} />
                )}
              </span>
              <div className="environment-item__body">
                <strong>{check.label}</strong>
                <span>{check.summary}</span>
                <small>{check.detail}</small>
              </div>
              {check.id === "project" && check.status !== "ok" ? (
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={onChooseProject}
                >
                  <FolderOpen size={13} />
                  选择
                </button>
              ) : check.id === "model" && check.status !== "ok" ? (
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={onOpenModels}
                >
                  配置
                </button>
              ) : check.fixAvailable && check.status !== "ok" ? (
                <button
                  type="button"
                  className="button button--ghost"
                  disabled={repairId === check.id}
                  onClick={() => onRepair(check)}
                >
                  <Wrench size={13} />
                  {repairId === check.id ? "处理中…" : "修复"}
                </button>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export function UpdateDiagnosticPanel({
  update,
  updateLoading,
  downloaded,
  downloadLoading,
  diagnostic,
  diagnosticLoading,
  onCheckUpdate,
  onDownload,
  onLaunch,
  onCollectDiagnostics,
}: {
  update?: UpdateInfo;
  updateLoading: boolean;
  downloaded?: { path: string; bytes: number };
  downloadLoading: boolean;
  diagnostic?: DiagnosticResult;
  diagnosticLoading: boolean;
  onCheckUpdate: () => void;
  onDownload: () => void;
  onLaunch: () => void;
  onCollectDiagnostics: () => void;
}) {
  return (
    <section className="product-card" aria-labelledby="update-title">
      <div className="product-card__header">
        <div>
          <p className="panel-eyebrow">MAINTENANCE</p>
          <h3 id="update-title">更新与诊断</h3>
          <p>检查发布版本，或生成不含密钥和会话正文的诊断包。</p>
        </div>
      </div>
      <div className="maintenance-actions">
        <button
          type="button"
          className="button button--secondary"
          disabled={updateLoading}
          onClick={onCheckUpdate}
        >
          <RefreshCw size={14} />
          {updateLoading ? "检查中…" : "检查更新"}
        </button>
        <button
          type="button"
          className="button button--secondary"
          disabled={diagnosticLoading}
          onClick={onCollectDiagnostics}
        >
          <ShieldAlert size={14} />
          {diagnosticLoading ? "生成中…" : "生成诊断包"}
        </button>
      </div>
      {update && (
        <div className="maintenance-result">
          <strong>{update.message}</strong>
          <small>
            当前版本 {update.currentVersion}
            {update.latestVersion ? ` · 最新版本 ${update.latestVersion}` : ""}
          </small>
          {update.updateAvailable && update.installerUrl && (
            <div className="maintenance-actions">
              {!downloaded ? (
                <button
                  type="button"
                  className="button button--primary"
                  disabled={downloadLoading}
                  onClick={onDownload}
                >
                  <Download size={14} />
                  {downloadLoading ? "下载中…" : "下载更新"}
                </button>
              ) : (
                <button
                  type="button"
                  className="button button--primary"
                  onClick={onLaunch}
                >
                  <ExternalLink size={14} />
                  启动安装程序
                </button>
              )}
              {update.releaseUrl && (
                <a href={update.releaseUrl} target="_blank" rel="noreferrer">
                  查看发布页
                </a>
              )}
            </div>
          )}
        </div>
      )}
      {diagnostic && (
        <p className="product-success">
          诊断包已生成：<code>{diagnostic.path}</code>
        </p>
      )}
    </section>
  );
}
const emptyMemory: ProjectMemoryInput = {
  enabled: false,
  purpose: "",
  techStack: "",
  rules: "",
  avoid: "",
  testCommand: "",
  preferredLanguage: "",
};
