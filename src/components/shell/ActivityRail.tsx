import {
  Activity,
  Boxes,
  ClipboardList,
  PlayCircle,
  MessageSquare,
  Paperclip,
  Settings,
  Sparkles,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export type ActivityId =
  | "demo"
  | "chat"
  | "tasks"
  | "runtime"
  | "skills"
  | "models"
  | "attachments"
  | "settings";

interface ActivityItem {
  id: ActivityId;
  label: string;
  icon: LucideIcon;
}

const activities: ActivityItem[] = [
  { id: "demo", label: "演示", icon: PlayCircle },
  { id: "chat", label: "聊天", icon: MessageSquare },
  { id: "tasks", label: "任务", icon: ClipboardList },
  { id: "runtime", label: "运行", icon: Activity },
  { id: "skills", label: "Skills", icon: Sparkles },
  { id: "models", label: "模型", icon: Boxes },
  { id: "attachments", label: "附件", icon: Paperclip },
  { id: "settings", label: "设置", icon: Settings },
];

interface Props {
  active: ActivityId;
  onChange: (activity: ActivityId) => void;
  attachmentCount: number;
  skillCount: number;
}

export function ActivityRail({
  active,
  onChange,
  attachmentCount,
  skillCount,
}: Props) {
  return (
    <nav className="activity-rail" aria-label="工作区">
      <div className="rail-brand" aria-label="CC Panel">
        <span aria-hidden="true">CC</span>
      </div>
      <div className="rail-items">
        {activities.map(({ id, label, icon: Icon }) => {
          const count =
            id === "attachments"
              ? attachmentCount
              : id === "skills"
                ? skillCount
                : 0;
          return (
            <button
              type="button"
              key={id}
              className="rail-item"
              data-active={active === id || undefined}
              aria-current={active === id ? "page" : undefined}
              aria-pressed={active === id}
              aria-label={label}
              title={label}
              onClick={() => onChange(id)}
            >
              <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
              <span className="rail-label">{label}</span>
              {count > 0 && <span className="rail-count">{count}</span>}
            </button>
          );
        })}
      </div>
      <div className="rail-footer" aria-hidden="true">
        <span className="rail-status-dot" />
      </div>
    </nav>
  );
}
