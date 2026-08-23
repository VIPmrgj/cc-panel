import {
  ArrowUp,
  FileCode2,
  Paperclip,
  Square,
  WandSparkles,
  X,
} from "lucide-react";
import { useEffect, useRef } from "react";
import type { AttachmentRecord, SkillRecord } from "../../api/dto";

interface Props {
  value: string;
  busy: boolean;
  queuedCount: number;
  queuedItems: Array<{ id: string; preview: string }>;
  onRemoveQueued: (id: string) => void;
  sessionActive: boolean;
  attachments: AttachmentRecord[];
  selectedSkills: SkillRecord[];
  ollamaAvailable: boolean;
  ollamaSelectedModel: string | null;
  enhancedPrompt: string | null;
  useEnhanced: boolean;
  showFinal: boolean;
  finalText: string | null;
  enhancing: boolean;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop: () => void;
  onAddFiles: () => void;
  onEnhance: () => void;
  onUseEnhanced: (value: boolean) => void;
  onToggleFinal: () => void;
}

export function ChatComposer({
  value,
  busy,
  queuedCount,
  queuedItems,
  onRemoveQueued,
  sessionActive,
  attachments,
  selectedSkills,
  ollamaAvailable,
  ollamaSelectedModel,
  enhancedPrompt,
  useEnhanced,
  showFinal,
  finalText,
  enhancing,
  onChange,
  onSend,
  onStop,
  onAddFiles,
  onEnhance,
  onUseEnhanced,
  onToggleFinal,
}: Props) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!busy) textareaRef.current?.focus();
  }, [busy]);

  const insertNewline = () => {
    const el = textareaRef.current;
    if (!el) return;
    const start = el.selectionStart ?? value.length;
    const end = el.selectionEnd ?? value.length;
    onChange(value.slice(0, start) + "\n" + value.slice(end));
    requestAnimationFrame(() => {
      el.selectionStart = el.selectionEnd = start + 1;
    });
  };

  return (
    <div className="chat-composer-shell">
      {queuedItems.length > 0 && (
        <div className="composer-queue" aria-label="待发送消息队列">
          <div className="composer-queue__header">
            <strong>待发送消息</strong>
            <span>{queuedItems.length} 条</span>
          </div>
          <ol>
            {queuedItems.map((item) => (
              <li key={item.id}>
                <span title={item.preview}>{item.preview}</span>
                <button
                  type="button"
                  aria-label={"取消排队：" + item.preview}
                  title="取消这条排队消息"
                  onClick={() => onRemoveQueued(item.id)}
                >
                  <X size={13} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ol>
        </div>
      )}
      {enhancedPrompt && (
        <div className="chat-enhance-candidate">
          <div className="chat-enhance-candidate__head">
            <span className="chat-enhance-candidate__label">
              Ollama 增强候选（本地改写）
            </span>
            <div
              className="segmented-control"
              aria-label="选择发送的 Prompt 版本"
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
          </div>
          <pre className="chat-enhance-candidate__text">{enhancedPrompt}</pre>
        </div>
      )}
      <div className="chat-composer" data-busy={busy || undefined}>
        {(attachments.length > 0 || selectedSkills.length > 0) && (
          <div className="composer-context" aria-label="本次请求的上下文">
            {selectedSkills.map((skill) => (
              <span key={skill.instanceId}>/{skill.displayName}</span>
            ))}
            {attachments.map((attachment) => (
              <span key={attachment.handle}>{attachment.name}</span>
            ))}
          </div>
        )}
        <label className="sr-only" htmlFor="chat-prompt">
          发送给 Claude Code
        </label>
        <textarea
          ref={textareaRef}
          id="chat-prompt"
          className="chat-composer__input"
          value={showFinal ? (finalText ?? value) : value}
          rows={4}
          readOnly={showFinal}
          placeholder="询问 Claude Code，或描述要完成的任务…"
          spellCheck
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing || showFinal) return;
            if (event.key === "Enter" && !event.ctrlKey && !event.shiftKey) {
              event.preventDefault();
              if (value.trim()) onSend();
            } else if (event.key === "Enter" && event.ctrlKey) {
              event.preventDefault();
              insertNewline();
            }
          }}
        />
        <div className="chat-composer__toolbar">
          <div className="chat-composer__tools">
            <button
              type="button"
              className="composer-tool"
              onClick={onAddFiles}
              disabled={busy}
            >
              <Paperclip size={15} aria-hidden="true" />
              <span>附件</span>
              {attachments.length > 0 && <b>{attachments.length}</b>}
            </button>
            <button
              type="button"
              className="composer-tool"
              onClick={onEnhance}
              disabled={
                busy ||
                enhancing ||
                !ollamaAvailable ||
                !ollamaSelectedModel ||
                !value.trim()
              }
              title={
                !ollamaAvailable
                  ? "Ollama 当前离线"
                  : !ollamaSelectedModel
                    ? "Ollama 在线但未选择本地模型，无法增强"
                    : "使用本地 Ollama 增强"
              }
            >
              <WandSparkles size={15} aria-hidden="true" />
              <span>{enhancing ? "增强中" : "增强"}</span>
            </button>
            <button
              type="button"
              className="composer-tool"
              onClick={onToggleFinal}
              disabled={busy || !value.trim()}
              aria-pressed={showFinal}
              title={
                showFinal
                  ? "当前显示最终提交的 Prompt，点击切回原文"
                  : "当前显示原文；点击查看最终提交的 Prompt 格式（始终按该格式提交）"
              }
            >
              <FileCode2 size={15} aria-hidden="true" />
              <span>{showFinal ? "原文" : "最终格式"}</span>
            </button>
          </div>
          <div className="chat-composer__send">
            <span className="composer-hint">
              <kbd>Enter</kbd> 发送 · <kbd>Ctrl</kbd>
              <kbd>Enter</kbd> 换行
            </span>
            {queuedCount > 0 && (
              <span className="composer-queue-status" aria-live="polite">
                已排队 {queuedCount} 条
              </span>
            )}
            {busy ? (
              <>
                <button
                  type="button"
                  className="send-button send-button--queue"
                  aria-label="排队发送"
                  disabled={!value.trim()}
                  onClick={onSend}
                >
                  <ArrowUp size={17} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className="send-button send-button--stop"
                  aria-label="停止生成"
                  onClick={onStop}
                >
                  <Square size={14} fill="currentColor" aria-hidden="true" />
                </button>
              </>
            ) : (
              <button
                type="button"
                className="send-button"
                aria-label={sessionActive ? "发送消息" : "开始会话并发送"}
                disabled={!value.trim()}
                onClick={onSend}
              >
                <ArrowUp size={17} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      </div>
      <p className="composer-disclaimer">
        Claude 可能犯错。执行命令和文件修改前请确认。
      </p>
    </div>
  );
}
