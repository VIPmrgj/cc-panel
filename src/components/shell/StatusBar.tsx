interface Props {
  project: string | null;
  skillCount: number;
  attachmentCount: number;
  ollamaOnline: boolean;
  message: string;
}

export function StatusBar({
  project,
  skillCount,
  attachmentCount,
  ollamaOnline,
  message,
}: Props) {
  return (
    <footer className="status-bar">
      <span title={project ?? "未选择项目"}>{project ?? "No project"}</span>
      <span>Skills {skillCount}</span>
      <span>附件 {attachmentCount}</span>
      <span className={ollamaOnline ? "status-online" : "status-offline"}>
        Ollama {ollamaOnline ? "online" : "offline"}
      </span>
      <span className="status-bar__message">{message}</span>
    </footer>
  );
}
