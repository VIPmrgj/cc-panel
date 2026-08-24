import { ChevronDown, Menu, PanelLeftClose, Plus } from "lucide-react";
import type { ModelProfile } from "../../api/dto";
import type { ChatRunState } from "../../state/chatReducer";

interface Props {
  title: string;
  profile: ModelProfile | null;
  status: ChatRunState;
  panelOpen: boolean;
  onTogglePanel: () => void;
  onSelectModel: () => void;
  onNewChat: () => void;
}

const statusLabels: Record<Props["status"], string> = {
  disconnected: "等待开始",
  starting: "正在启动",
  thinking: "正在思考",
  "tool-running": "正在执行工具",
  "awaiting-permission": "等待确认",
  stopping: "正在停止",
  stalled: "可能卡住",
  recovering: "正在恢复",
  idle: "已就绪",
  ended: "可继续对话",
  failed: "上一回合失败",
};

export function ChatHeader({
  title,
  profile,
  status,
  panelOpen,
  onTogglePanel,
  onSelectModel,
  onNewChat,
}: Props) {
  return (
    <header className="chat-header">
      <div className="chat-header__leading">
        <button
          type="button"
          className="header-icon-button mobile-panel-toggle"
          aria-label="打开当前面板"
          onClick={onTogglePanel}
        >
          <Menu size={18} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="header-icon-button desktop-panel-toggle"
          aria-label={panelOpen ? "收起侧栏" : "展开侧栏"}
          aria-pressed={panelOpen}
          onClick={onTogglePanel}
        >
          <PanelLeftClose size={17} aria-hidden="true" />
        </button>
        <div className="chat-header__title">
          <h1>{title || "新对话"}</h1>
          <button
            type="button"
            className="model-pill"
            title="选择模型配置"
            onClick={onSelectModel}
          >
            <span>
              {profile
                ? `${profile.providerName} · ${profile.modelId}`
                : "选择模型配置"}
            </span>
            <ChevronDown size={12} aria-hidden="true" />
          </button>
        </div>
      </div>
      <div className="chat-header__actions">
        <span className={`session-state session-state--${status}`}>
          <i aria-hidden="true" />
          {statusLabels[status]}
        </span>
        <button type="button" className="header-action" onClick={onNewChat}>
          <Plus size={15} aria-hidden="true" />
          <span>新对话</span>
        </button>
      </div>
    </header>
  );
}
