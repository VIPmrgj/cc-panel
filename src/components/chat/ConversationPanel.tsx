import { useMemo, useState } from "react";
import {
  Archive,
  Clock3,
  Copy,
  MessageSquarePlus,
  Pencil,
  Search,
  Star,
  Trash2,
} from "lucide-react";
import type { ConversationSummary } from "../../api/dto";

interface Props {
  conversations: ConversationSummary[];
  activeSessionId: string | null;
  loading?: boolean;
  onSelect: (conversation: ConversationSummary) => void;
  onDelete: (conversation: ConversationSummary) => void;
  onNew: () => void;
  onRename: (conversation: ConversationSummary, title: string) => void;
  onFavorite: (conversation: ConversationSummary) => void;
  onArchive: (conversation: ConversationSummary) => void;
  onExport: (conversation: ConversationSummary) => void;
}

export function ConversationPanel({
  conversations,
  activeSessionId,
  loading = false,
  onSelect,
  onDelete,
  onNew,
  onRename,
  onFavorite,
  onArchive,
  onExport,
}: Props) {
  const [search, setSearch] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const visible = useMemo(
    () =>
      conversations
        .filter(
          (conversation) =>
            (showArchived || !conversation.archived) &&
            [
              conversation.title,
              conversation.projectPath,
              conversation.modelId,
            ].some((value) =>
              value?.toLowerCase().includes(search.trim().toLowerCase()),
            ),
        )
        .sort(
          (a, b) =>
            Number(b.favorite) - Number(a.favorite) ||
            b.updatedAtMs - a.updatedAtMs,
        ),
    [conversations, search, showArchived],
  );
  return (
    <div className="conversation-panel">
      <div className="context-panel__header">
        <div>
          <p className="panel-eyebrow">WORKSPACE</p>
          <h2>对话</h2>
        </div>
        <button
          type="button"
          className="panel-icon-button"
          aria-label="新对话"
          onClick={onNew}
        >
          <MessageSquarePlus size={16} aria-hidden="true" />
        </button>
      </div>
      <label className="panel-search">
        <Search size={14} aria-hidden="true" />
        <span className="sr-only">搜索对话</span>
        <input
          type="search"
          placeholder="搜索对话、项目或模型"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
      </label>
      <label className="conversation-archive-toggle">
        <input
          type="checkbox"
          checked={showArchived}
          onChange={(event) => setShowArchived(event.target.checked)}
        />
        显示归档
      </label>
      <div className="conversation-list" aria-busy={loading || undefined}>
        {loading ? (
          <p className="panel-muted">正在读取历史…</p>
        ) : visible.length === 0 ? (
          <div className="panel-empty">
            <Clock3 size={20} aria-hidden="true" />
            <p>{search ? "没有匹配的对话" : "还没有历史对话"}</p>
            <button type="button" onClick={onNew}>
              开始第一段对话
            </button>
          </div>
        ) : (
          visible.map((conversation) => {
            const editing = editingId === conversation.sessionId;
            return (
              <div
                className="conversation-item-wrap"
                data-active={
                  conversation.sessionId === activeSessionId || undefined
                }
                key={conversation.sessionId}
              >
                <button
                  type="button"
                  className="conversation-item"
                  aria-current={
                    conversation.sessionId === activeSessionId
                      ? "page"
                      : undefined
                  }
                  onClick={() => onSelect(conversation)}
                >
                  {editing ? (
                    <input
                      autoFocus
                      value={editingTitle}
                      onClick={(event) => event.stopPropagation()}
                      onChange={(event) => setEditingTitle(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && editingTitle.trim()) {
                          onRename(conversation, editingTitle.trim());
                          setEditingId(null);
                        }
                        if (event.key === "Escape") setEditingId(null);
                      }}
                    />
                  ) : (
                    <span className="conversation-item__title">
                      {conversation.favorite && (
                        <Star
                          size={12}
                          fill="currentColor"
                          aria-label="已收藏"
                        />
                      )}
                      {conversation.title || "未命名对话"}
                    </span>
                  )}
                  <span className="conversation-item__meta">
                    {formatRelativeTime(conversation.updatedAtMs)} ·{" "}
                    {conversation.modelId ?? "默认模型"}
                  </span>
                </button>
                <div className="conversation-item__actions">
                  <button
                    type="button"
                    aria-label="收藏对话"
                    title={conversation.favorite ? "取消收藏" : "收藏"}
                    onClick={(event) => {
                      event.stopPropagation();
                      onFavorite(conversation);
                    }}
                  >
                    <Star
                      size={13}
                      fill={conversation.favorite ? "currentColor" : "none"}
                    />
                  </button>
                  <button
                    type="button"
                    aria-label="重命名对话"
                    title="重命名"
                    onClick={(event) => {
                      event.stopPropagation();
                      setEditingId(conversation.sessionId);
                      setEditingTitle(conversation.title);
                    }}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    type="button"
                    aria-label="导出对话"
                    title="导出为 Markdown"
                    onClick={(event) => {
                      event.stopPropagation();
                      onExport(conversation);
                    }}
                  >
                    <Copy size={13} />
                  </button>
                  <button
                    type="button"
                    aria-label={conversation.archived ? "取消归档" : "归档对话"}
                    title={conversation.archived ? "取消归档" : "归档"}
                    onClick={(event) => {
                      event.stopPropagation();
                      onArchive(conversation);
                    }}
                  >
                    <Archive size={13} />
                  </button>
                  <button
                    type="button"
                    aria-label="删除对话"
                    title="删除该对话记录"
                    onClick={(event) => {
                      event.stopPropagation();
                      onDelete(conversation);
                    }}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

function formatRelativeTime(timestamp: number) {
  const delta = Date.now() - timestamp;
  if (delta < 60000) return "刚刚";
  if (delta < 3600000) return `${Math.floor(delta / 60000)} 分钟前`;
  if (delta < 86400000) return `${Math.floor(delta / 3600000)} 小时前`;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
  }).format(timestamp);
}
