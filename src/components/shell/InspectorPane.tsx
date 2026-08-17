import { useState } from "react";
import { Activity, Files, Gauge, ScrollText } from "lucide-react";
import type {
  AttachmentRecord,
  CompositionResult,
  OllamaStatus,
} from "../../api/dto";
import { AttachmentList } from "../attachments/AttachmentList";
import { Notice } from "../common/Notice";

interface Props {
  attachments: AttachmentRecord[];
  preview: CompositionResult | null;
  previewStale: boolean;
  ollama: OllamaStatus;
  operationMessage: string;
  nativeNotificationsEnabled: boolean;
  notificationSaving: boolean;
  onSetNativeNotifications: (enabled: boolean) => void;
  onRemoveAttachment: (handle: string) => void;
  onMoveAttachment: (handle: string, direction: -1 | 1) => void;
}

type Tab = "attachments" | "preview" | "status";

export function InspectorPane({
  attachments,
  preview,
  previewStale,
  ollama,
  operationMessage,
  nativeNotificationsEnabled,
  notificationSaving,
  onSetNativeNotifications,
  onRemoveAttachment,
  onMoveAttachment,
}: Props) {
  const [tab, setTab] = useState<Tab>("attachments");
  return (
    <aside className="inspector-pane" aria-label="附件、预览和状态">
      <div className="inspector-tabs" aria-label="检查器">
        <button
          aria-pressed={tab === "attachments"}
          onClick={() => setTab("attachments")}
        >
          <Files size={14} aria-hidden="true" />
          附件 <span>{attachments.length}</span>
        </button>
        <button
          aria-pressed={tab === "preview"}
          onClick={() => setTab("preview")}
        >
          <ScrollText size={14} aria-hidden="true" />
          预览
        </button>
        <button
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
            onRemove={onRemoveAttachment}
            onMove={onMoveAttachment}
          />
        )}
        {tab === "preview" && (
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
                <strong>尚未构建预览</strong>
                <p>统计为精确字符与字节，不伪造模型 token 数。</p>
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

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value.toLocaleString()}</strong>
    </div>
  );
}
