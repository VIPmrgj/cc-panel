import {
  ChevronDown,
  ChevronUp,
  File,
  FileImage,
  Eye,
  FileText,
  Trash2,
} from "lucide-react";
import type { AttachmentRecord } from "../../api/dto";
import { Button } from "../common/Button";

interface Props {
  attachments: AttachmentRecord[];
  onRemove: (handle: string) => void;
  onMove: (handle: string, direction: -1 | 1) => void;
  onPreview: (handle: string) => void;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

export function AttachmentList({
  attachments,
  onRemove,
  onMove,
  onPreview,
}: Props) {
  if (!attachments.length) {
    return (
      <div className="empty-state">
        <File size={28} aria-hidden="true" />
        <strong>暂无附件</strong>
        <p>可点击“添加附件”或把文件拖入窗口。</p>
      </div>
    );
  }
  return (
    <div className="attachment-list">
      {attachments.map((attachment, index) => {
        const Icon = attachment.kind === "image" ? FileImage : FileText;
        return (
          <article className="attachment-item" key={attachment.handle}>
            <Icon size={17} aria-hidden="true" />
            <div className="attachment-item__body">
              <strong>{attachment.name}</strong>
              <span>
                {attachment.kind.toUpperCase()} ·{" "}
                {formatBytes(attachment.rawBytes)}
              </span>
              {attachment.warnings.map((warning) => (
                <small key={warning}>{warning}</small>
              ))}
            </div>
            <div className="attachment-item__actions">
              <Button
                variant="ghost"
                className="icon-button icon-button--small"
                icon={<Eye size={14} />}
                aria-label={"查看 " + attachment.name}
                title="查看附件内容"
                onClick={() => onPreview(attachment.handle)}
              >
                <span className="sr-only">查看内容</span>
              </Button>
              <Button
                variant="ghost"
                className="icon-button icon-button--small"
                icon={<ChevronUp size={14} />}
                aria-label={`上移 ${attachment.name}`}
                title="上移"
                disabled={index === 0}
                onClick={() => onMove(attachment.handle, -1)}
              >
                <span className="sr-only">上移</span>
              </Button>
              <Button
                variant="ghost"
                className="icon-button icon-button--small"
                icon={<ChevronDown size={14} />}
                aria-label={`下移 ${attachment.name}`}
                title="下移"
                disabled={index === attachments.length - 1}
                onClick={() => onMove(attachment.handle, 1)}
              >
                <span className="sr-only">下移</span>
              </Button>
              <Button
                variant="ghost"
                className="icon-button icon-button--small danger-icon"
                icon={<Trash2 size={14} />}
                aria-label={`移除 ${attachment.name}`}
                title="移除"
                onClick={() => onRemove(attachment.handle)}
              >
                <span className="sr-only">移除</span>
              </Button>
            </div>
          </article>
        );
      })}
    </div>
  );
}
