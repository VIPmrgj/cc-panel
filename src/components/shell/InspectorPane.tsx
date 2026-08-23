import { useEffect, useState } from "react";
import {
  Activity,
  Eye,
  Files,
  FileText,
  Gauge,
  Image as ImageIcon,
  ScrollText,
} from "lucide-react";
import type {
  AttachmentPreview,
  AttachmentRecord,
  CompositionResult,
  OllamaStatus,
} from "../../api/dto";
import { AttachmentList } from "../attachments/AttachmentList";
import { Notice } from "../common/Notice";

interface Props {
  attachments: AttachmentRecord[];
  attachmentPreview: AttachmentPreview | null;
  attachmentPreviewLoading: boolean;
  promptPreviewRequest: number;
  preview: CompositionResult | null;
  previewStale: boolean;
  ollama: OllamaStatus;
  operationMessage: string;
  nativeNotificationsEnabled: boolean;
  notificationSaving: boolean;
  onSetNativeNotifications: (enabled: boolean) => void;
  onPreviewAttachment: (handle: string) => void;
  onRemoveAttachment: (handle: string) => void;
  onMoveAttachment: (handle: string, direction: -1 | 1) => void;
}

type Tab = "attachments" | "attachment-preview" | "prompt-preview" | "status";

export function InspectorPane({
  attachments,
  attachmentPreview,
  attachmentPreviewLoading,
  promptPreviewRequest,
  preview,
  previewStale,
  ollama,
  operationMessage,
  nativeNotificationsEnabled,
  notificationSaving,
  onSetNativeNotifications,
  onPreviewAttachment,
  onRemoveAttachment,
  onMoveAttachment,
}: Props) {
  const [tab, setTab] = useState<Tab>("attachments");
  useEffect(() => {
    if (promptPreviewRequest > 0) setTab("prompt-preview");
  }, [promptPreviewRequest]);
  const showAttachmentPreview = (handle: string) => {
    setTab("attachment-preview");
    onPreviewAttachment(handle);
  };

  return (
    <aside className="inspector-pane" aria-label="附件、预览和状态">
      <div className="inspector-tabs" aria-label="检查器">
        <button
          type="button"
          aria-pressed={tab === "attachments"}
          onClick={() => setTab("attachments")}
        >
          <Files size={14} aria-hidden="true" />
          附件 <span>{attachments.length}</span>
        </button>
        <button
          type="button"
          aria-pressed={tab === "attachment-preview"}
          onClick={() => setTab("attachment-preview")}
        >
          <Eye size={14} aria-hidden="true" />
          文件预览
        </button>
        <button
          type="button"
          aria-pressed={tab === "prompt-preview"}
          onClick={() => setTab("prompt-preview")}
        >
          <ScrollText size={14} aria-hidden="true" />
          Prompt 预览
        </button>
        <button
          type="button"
          aria-pressed={tab === "status"}
          onClick={() => setTab("status")}
        >
          <Activity size={14} aria-hidden="true" />
          状态
        </button>
      </div>
      <div className="inspector-content">
        {tab === "attachments" && (
          <AttachmentList
            attachments={attachments}
            onPreview={showAttachmentPreview}
            onRemove={onRemoveAttachment}
            onMove={onMoveAttachment}
          />
        )}
        {tab === "attachment-preview" && (
          <AttachmentPreviewPanel
            preview={attachmentPreview}
            loading={attachmentPreviewLoading}
          />
        )}
        {tab === "prompt-preview" && (
          <div className="preview-panel">
            {previewStale && preview && (
              <Notice tone="warning">输入已变化，当前预览已过期。</Notice>
            )}
            {preview ? (
              <>
                {preview.warnings.map((warning) => (
                  <Notice tone="warning" key={warning}>
                    {warning}
                  </Notice>
                ))}
                <p className="preview-note">
                  这是发送前的最终 Prompt：原文、Skill 和附件会被组合成 XML
                  后发送给 Claude，不是单个文件的预览。
                </p>
                <pre className="prompt-preview">{preview.text}</pre>
                <div className="stats-grid" aria-label="最终 Prompt 统计">
                  <Stat label="字符" value={preview.characters} />
                  <Stat label="UTF-8 字节" value={preview.utf8Bytes} />
                  <Stat label="行" value={preview.lines} />
                  <Stat label="Skills" value={preview.skillCount} />
                  <Stat label="附件" value={preview.attachmentCount} />
                </div>
              </>
            ) : (
              <div className="empty-state">
                <Gauge size={28} aria-hidden="true" />
                <strong>尚未构建 Prompt 预览</strong>
                <p>先在输入框中输入内容或添加附件，再点击“最终格式”。</p>
              </div>
            )}
          </div>
        )}
        {tab === "status" && (
          <div className="status-panel">
            <Notice tone={ollama.online ? "success" : "warning"}>
              {ollama.message}
            </Notice>
            <dl>
              <div>
                <dt>Ollama 地址</dt>
                <dd>{ollama.baseUrl}</dd>
              </div>
              <div>
                <dt>当前本地模型</dt>
                <dd>{ollama.selectedModel ?? "未选择"}</dd>
              </div>
              <div>
                <dt>自动压缩策略</dt>
                <dd>Auto-compact: 272k</dd>
              </div>
              <div>
                <dt>附件存储</dt>
                <dd>仅内存</dd>
              </div>
              <div>
                <dt>Skill 应用</dt>
                <dd>/reload-plugins 或重启</dd>
              </div>
            </dl>
            <label className="status-setting">
              <span>
                <strong>复制后发送系统通知</strong>
                <small>首次开启时会请求操作系统权限。</small>
              </span>
              <input
                type="checkbox"
                checked={nativeNotificationsEnabled}
                disabled={notificationSaving}
                onChange={(event) =>
                  onSetNativeNotifications(event.target.checked)
                }
              />
            </label>
            {operationMessage && <Notice>{operationMessage}</Notice>}
          </div>
        )}
      </div>
    </aside>
  );
}

function AttachmentPreviewPanel({
  preview,
  loading,
}: {
  preview: AttachmentPreview | null;
  loading: boolean;
}) {
  if (loading) {
    return (
      <div className="empty-state">
        <Eye size={28} aria-hidden="true" />
        <strong>正在读取附件</strong>
        <p>内容只从内存中的安全句柄读取，不会重新访问任意路径。</p>
      </div>
    );
  }

  if (!preview) {
    return (
      <div className="empty-state">
        <FileText size={28} aria-hidden="true" />
        <strong>还没有选择文件</strong>
        <p>在“附件”列表中点击眼睛按钮查看单个文件。</p>
      </div>
    );
  }

  const { attachment } = preview;
  return (
    <div className="attachment-preview-panel">
      <header className="attachment-preview-header">
        {attachment.kind === "image" ? (
          <ImageIcon size={18} aria-hidden="true" />
        ) : (
          <FileText size={18} aria-hidden="true" />
        )}
        <div>
          <strong>{attachment.name}</strong>
          <span>
            {attachment.kind.toUpperCase()} · {attachment.mime} ·{" "}
            {formatBytes(attachment.rawBytes)}
          </span>
        </div>
      </header>
      {preview.dataUrl && (
        <img
          className="attachment-image-preview"
          src={preview.dataUrl}
          alt={"附件 " + attachment.name + " 的预览"}
        />
      )}
      <p className="preview-note">
        {attachment.kind === "image"
          ? "这是按需生成的图片缩略图；原始图片不会写入磁盘，也不会把本地路径交给前端。"
          : attachment.kind === "pdf"
            ? "这是从 PDF 提取出的文本内容；扫描型 PDF 目前不提供 OCR。"
            : "这是附件导入后保存在内存中的文本内容。"}
      </p>
      {preview.truncated && (
        <Notice tone="warning">
          文件内容较长，当前只显示前 50,000 个字符；发送时仍使用完整提取内容。
        </Notice>
      )}
      <pre className="attachment-content-preview">{preview.content}</pre>
    </div>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KiB";
  return (bytes / 1024 / 1024).toFixed(1) + " MiB";
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value.toLocaleString()}</strong>
    </div>
  );
}
