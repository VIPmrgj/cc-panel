import {
  FilePlus2,
  Sparkles,
  X,
  Copy,
  Eye,
  PanelLeft,
  PanelRight,
} from "lucide-react";
import type { OllamaStatus, SkillRecord } from "../../api/dto";
import { Button } from "../common/Button";

interface Props {
  originalPrompt: string;
  enhancedPrompt: string | null;
  useEnhanced: boolean;
  selectedSkills: SkillRecord[];
  ollama: OllamaStatus;
  enhancing: boolean;
  composing: boolean;
  copying: boolean;
  canCompose: boolean;
  onPromptChange: (value: string) => void;
  onUseEnhanced: (value: boolean) => void;
  onEnhance: () => void;
  onAddFiles: () => void;
  onPreview: () => void;
  onCopy: () => void;
  onRemoveSkill: (instanceId: string) => void;
  onOpenLeft: () => void;
  onOpenRight: () => void;
}

export function ComposerPane({
  originalPrompt,
  enhancedPrompt,
  useEnhanced,
  selectedSkills,
  ollama,
  enhancing,
  composing,
  copying,
  canCompose,
  onPromptChange,
  onUseEnhanced,
  onEnhance,
  onAddFiles,
  onPreview,
  onCopy,
  onRemoveSkill,
  onOpenLeft,
  onOpenRight,
}: Props) {
  return (
    <main className="composer-pane" id="main-content">
      <header className="composer-header">
        <Button
          variant="ghost"
          className="icon-button responsive-left-trigger"
          icon={<PanelLeft size={17} />}
          aria-label="打开模型和 Skill 面板"
          title="模型和 Skills"
          onClick={onOpenLeft}
        >
          <span className="sr-only">模型和 Skills</span>
        </Button>
        <div>
          <p className="section-kicker">PROMPT WORKSPACE</p>
          <h1>Prompt Composer</h1>
        </div>
        <Button
          variant="ghost"
          className="icon-button responsive-right-trigger"
          icon={<PanelRight size={17} />}
          aria-label="打开附件和预览面板"
          title="附件和预览"
          onClick={onOpenRight}
        >
          <span className="sr-only">附件和预览</span>
        </Button>
      </header>

      <div className="selected-skills" aria-label="已选择的 Skills">
        {selectedSkills.length === 0 ? (
          <span className="selected-skills__empty">尚未加入 Skill</span>
        ) : (
          selectedSkills.map((skill) => (
            <span className="skill-chip" key={skill.instanceId}>
              {skill.displayName}
              <button
                aria-label={`移除 ${skill.displayName}`}
                title="从 Prompt 移除"
                onClick={() => onRemoveSkill(skill.instanceId)}
              >
                <X size={12} aria-hidden="true" />
              </button>
            </span>
          ))
        )}
      </div>

      <section className="editor-card" aria-labelledby="prompt-label">
        <div className="editor-toolbar">
          <label id="prompt-label" htmlFor="prompt-editor">
            原始 Prompt
          </label>
          <span>
            {new TextEncoder().encode(originalPrompt).length.toLocaleString()}{" "}
            bytes
          </span>
        </div>
        <textarea
          id="prompt-editor"
          className="prompt-editor"
          value={originalPrompt}
          onChange={(event) => onPromptChange(event.target.value)}
          placeholder="描述要交给 Claude Code 的任务。Skill 与附件会在后端组合，不会改写这里的原文。"
          spellCheck
        />
      </section>

      <section className="enhancement-card" aria-labelledby="enhance-title">
        <div className="enhancement-header">
          <div>
            <p className="section-kicker">LOCAL REWRITE</p>
            <h2 id="enhance-title">Ollama 增强</h2>
          </div>
          <div className="enhancement-actions">
            <span
              className={`status-dot ${ollama.online ? "status-dot--online" : "status-dot--offline"}`}
            >
              {ollama.online ? (ollama.selectedModel ?? "在线") : "离线"}
            </span>
            <Button
              icon={<Sparkles size={15} />}
              busy={enhancing}
              disabled={
                !originalPrompt.trim() ||
                !ollama.online ||
                !ollama.selectedModel
              }
              title={
                !ollama.online || !ollama.selectedModel
                  ? "Ollama 离线或未选择本地模型，无法改写"
                  : undefined
              }
              onClick={onEnhance}
            >
              增强
            </Button>
          </div>
        </div>
        <p className="field-help">
          仅本地 Ollama 改写（需先运行 Ollama
          并选择模型），不联网搜索；失败时原文保持不变。
          这里的「增强」与下方「构建预览/复制最终 Prompt」的 XML 组装是两回事。
        </p>
        {enhancedPrompt && (
          <div className="candidate-panel">
            <div
              className="segmented-control"
              aria-label="选择最终 Prompt 版本"
            >
              <button
                aria-pressed={!useEnhanced}
                data-active={!useEnhanced || undefined}
                onClick={() => onUseEnhanced(false)}
              >
                使用原文
              </button>
              <button
                aria-pressed={useEnhanced}
                data-active={useEnhanced || undefined}
                onClick={() => onUseEnhanced(true)}
              >
                使用增强版
              </button>
            </div>
            <pre>{enhancedPrompt}</pre>
          </div>
        )}
      </section>

      <div className="composer-spacer" />

      <div className="composer-actionbar">
        <div className="composer-secondary-actions">
          <Button icon={<FilePlus2 size={16} />} onClick={onAddFiles}>
            添加附件
          </Button>
          <Button
            icon={<Eye size={16} />}
            busy={composing}
            disabled={!canCompose}
            onClick={onPreview}
          >
            构建预览
          </Button>
        </div>
        <Button
          variant="primary"
          className="copy-button"
          icon={<Copy size={16} />}
          busy={copying}
          disabled={!canCompose}
          onClick={onCopy}
        >
          复制最终 Prompt
        </Button>
      </div>
    </main>
  );
}
