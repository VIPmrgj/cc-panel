import {
  Bell,
  Bot,
  Check,
  FolderOpen,
  Gauge,
  RotateCw,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import type { BootstrapResponse } from "../../api/dto";
import type { ExperienceMode } from "../../state/experienceMode";
import type { AppTheme } from "../../state/theme";
import type {
  DiagnosticResult,
  EnvironmentReport,
  ProjectMemory,
  ProjectMemoryInput,
  UpdateInfo,
  DownloadedUpdate,
} from "../../api/dto";
import {
  EnvironmentPanel,
  ProjectMemoryPanel,
  UpdateDiagnosticPanel,
} from "../product/ProductPanels";

interface Props {
  bootstrap: BootstrapResponse;
  experienceMode: ExperienceMode;
  theme: AppTheme;
  notificationSaving: boolean;
  ollamaSaving?: boolean;
  onExperienceModeChange: (mode: ExperienceMode) => void;
  onThemeChange: (theme: AppTheme) => void;
  onOpenOnboarding: () => void;
  onSetNativeNotifications: (enabled: boolean) => void;
  onSelectOllamaModel: (model: string | null) => void;
  projectMemory?: ProjectMemory | null;
  projectMemorySaving?: boolean;
  onSaveProjectMemory?: (input: ProjectMemoryInput) => void;
  environmentReport?: EnvironmentReport;
  environmentLoading?: boolean;
  environmentRepairId?: string | null;
  onRunEnvironmentCheck?: () => void;
  onRepairEnvironmentCheck?: (
    check: import("../../api/dto").EnvironmentCheck,
  ) => void;
  onChooseProject?: () => void;
  onOpenModels?: () => void;
  updateInfo?: UpdateInfo;
  updateLoading?: boolean;
  downloadedUpdate?: DownloadedUpdate;
  downloadLoading?: boolean;
  diagnosticResult?: DiagnosticResult;
  diagnosticLoading?: boolean;
  onCheckUpdate?: () => void;
  onDownloadUpdate?: () => void;
  onLaunchUpdate?: () => void;
  onCollectDiagnostics?: () => void;
}

export function SettingsPanel({
  bootstrap,
  experienceMode,
  theme,
  notificationSaving,
  ollamaSaving = false,
  onExperienceModeChange,
  onThemeChange,
  onOpenOnboarding,
  onSetNativeNotifications,
  onSelectOllamaModel,
  projectMemory,
  projectMemorySaving = false,
  onSaveProjectMemory,
  environmentReport,
  environmentLoading = false,
  environmentRepairId = null,
  onRunEnvironmentCheck,
  onRepairEnvironmentCheck,
  onChooseProject,
  onOpenModels,
  updateInfo,
  updateLoading = false,
  downloadedUpdate,
  downloadLoading = false,
  diagnosticResult,
  diagnosticLoading = false,
  onCheckUpdate,
  onDownloadUpdate,
  onLaunchUpdate,
  onCollectDiagnostics,
}: Props) {
  const project = bootstrap.preferences.selectedProjectRoot;
  const ollama = bootstrap.ollama;
  return (
    <section className="settings-panel" aria-labelledby="settings-panel-title">
      <div className="context-panel__header">
        <div>
          <p className="panel-eyebrow">RUNTIME</p>
          <h2 id="settings-panel-title">设置</h2>
        </div>
      </div>
      <div className="settings-panel__body">
        <div className="settings-panel__group">
          <div className="settings-panel__group-title">界面主题</div>
          <div className="theme-choice" role="group" aria-label="界面主题">
            <button
              type="button"
              className="theme-choice__item"
              aria-pressed={theme === "light"}
              onClick={() => onThemeChange("light")}
              title="灰色文字和淡白背景，适合日常使用。"
            >
              <strong>淡白主题</strong>
              <small>灰色文字、淡白背景。</small>
            </button>
            <button
              type="button"
              className="theme-choice__item"
              aria-pressed={theme === "night"}
              onClick={() => onThemeChange("night")}
              title="深色背景，适合低光环境。"
            >
              <strong>夜间模式</strong>
              <small>深色背景，减少环境亮度。</small>
            </button>
          </div>
        </div>
        <div className="settings-panel__group">
          <div className="settings-panel__group-title">显示体验</div>
          <div className="experience-choice" role="group" aria-label="显示体验">
            <button
              type="button"
              className="experience-choice__item"
              aria-pressed={experienceMode === "guided"}
              onClick={() => onExperienceModeChange("guided")}
              title="默认收起复杂选项，并在关键位置提供解释。"
            >
              <strong>引导体验</strong>
              <small>更少打扰，复杂选项仍可随时找到。</small>
            </button>
            <button
              type="button"
              className="experience-choice__item"
              aria-pressed={experienceMode === "complete"}
              onClick={() => onExperienceModeChange("complete")}
              title="默认显示更多控制项，适合快速调整细节。"
            >
              <strong>完整体验</strong>
              <small>更多控制项默认可见。</small>
            </button>
          </div>
          <button
            type="button"
            className="button button--ghost settings-panel__guide-button"
            onClick={onOpenOnboarding}
          >
            <RotateCw size={14} aria-hidden="true" />
            重新打开使用引导
          </button>
        </div>
        <SettingRow icon={FolderOpen} label="项目目录">
          {project?.path ?? "未选择"}
        </SettingRow>
        <SettingRow icon={Bot} label="Claude Code">
          {bootstrap.claudeCodeVersion ?? "未检测到"}
        </SettingRow>
        <SettingRow icon={Gauge} label="自动压缩策略">
          Auto-compact: 272k
        </SettingRow>
        <div
          className="settings-panel__group"
          title="使用本地 Ollama 整理指令，原始指令仍会保留。"
        >
          <div className="settings-panel__group-title">
            <Sparkles size={14} aria-hidden="true" />
            本地 Prompt 优化
          </div>
          <label className="settings-panel__select">
            <span>优化模型</span>
            <select
              aria-label="本地 Prompt 优化模型"
              value={ollama.selectedModel ?? ""}
              disabled={ollamaSaving || !ollama.models.length}
              onChange={(event) =>
                onSelectOllamaModel(event.target.value || null)
              }
            >
              <option value="">
                {ollama.online ? "关闭本地优化" : "Ollama 未连接"}
              </option>
              {ollama.models.map((model) => (
                <option key={model.name} value={model.name}>
                  {model.name}
                </option>
              ))}
            </select>
          </label>
          <small className="settings-panel__help">
            {ollama.online
              ? "只在你点击“增强”时调用本地模型，不会替换原始指令。"
              : "启动 Ollama 后重新打开这里即可选择模型。"}
          </small>
        </div>
        <div
          className="settings-panel__auto-approval"
          title="未识别到删除、提权、发布或外部传输特征的操作会自动允许。"
        >
          <span className="settings-panel__icon" aria-hidden="true">
            <Check size={15} />
          </span>
          <span>
            <strong>低风险操作自动允许</strong>
            <small>删除、提权、发布和外部传输等高风险操作仍需你确认。</small>
          </span>
        </div>
        <SettingRow icon={ShieldCheck} label="安全边界">
          附件仅保存在内存；提供商密钥不会返回界面。
        </SettingRow>
        <label className="settings-panel__toggle">
          <span className="settings-panel__icon" aria-hidden="true">
            <Bell size={15} />
          </span>
          <span>
            <strong>复制后发送系统通知</strong>
            <small>首次开启时会请求操作系统权限。</small>
          </span>
          <input
            type="checkbox"
            checked={bootstrap.preferences.nativeNotificationsEnabled}
            disabled={notificationSaving}
            onChange={(event) => onSetNativeNotifications(event.target.checked)}
          />
        </label>
        <div className="settings-product-groups">
          <details className="settings-panel__disclosure">
            <summary>
              项目记忆 <small>让 Agent 记住项目长期规则</small>
            </summary>
            <ProjectMemoryPanel
              memory={projectMemory}
              saving={projectMemorySaving}
              onSave={onSaveProjectMemory ?? (() => undefined)}
            />
          </details>
          <details className="settings-panel__disclosure">
            <summary>
              环境自检 <small>检查运行条件并尝试修复</small>
            </summary>
            <EnvironmentPanel
              report={environmentReport}
              loading={environmentLoading}
              repairId={environmentRepairId}
              onRefresh={onRunEnvironmentCheck ?? (() => undefined)}
              onRepair={onRepairEnvironmentCheck ?? (() => undefined)}
              onChooseProject={onChooseProject ?? (() => undefined)}
              onOpenModels={onOpenModels ?? (() => undefined)}
            />
          </details>
          <details className="settings-panel__disclosure">
            <summary>
              更新与诊断 <small>获取新版本或生成安全诊断包</small>
            </summary>
            <UpdateDiagnosticPanel
              update={updateInfo}
              updateLoading={updateLoading}
              downloaded={downloadedUpdate}
              downloadLoading={downloadLoading}
              diagnostic={diagnosticResult}
              diagnosticLoading={diagnosticLoading}
              onCheckUpdate={onCheckUpdate ?? (() => undefined)}
              onDownload={onDownloadUpdate ?? (() => undefined)}
              onLaunch={onLaunchUpdate ?? (() => undefined)}
              onCollectDiagnostics={onCollectDiagnostics ?? (() => undefined)}
            />
          </details>
        </div>{" "}
      </div>
    </section>
  );
}

function SettingRow({
  icon: Icon,
  label,
  children,
}: {
  icon: typeof FolderOpen;
  label: string;
  children: string;
}) {
  return (
    <div className="settings-panel__row">
      <span className="settings-panel__icon" aria-hidden="true">
        <Icon size={15} />
      </span>
      <span>
        <strong>{label}</strong>
        <small title={children}>{children}</small>
      </span>
    </div>
  );
}
