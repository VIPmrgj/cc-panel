import { Bot, UserRound } from "lucide-react";
import { memo, useEffect, useRef } from "react";
import { extractDisplayedUserPrompt } from "./promptDisplay";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import type { ChatMessage } from "../../api/dto";
import type { ActiveTool } from "../../state/chatReducer";
import {
  ActiveToolCard,
  BlockCard,
  PermissionCard,
  type PermissionDecision,
} from "./ChatCards";

interface Props {
  messages: ChatMessage[];
  activePermission?: ChatMessage | null;
  permissionBusy?: boolean;
  activeTool?: ActiveTool | null;
  onPermission: (requestId: string, behavior: PermissionDecision) => void;
  onRetryPermission: (requestId: string) => void;
}

/** Distance from the bottom still treated as "pinned to the bottom". */
const BOTTOM_PIN_THRESHOLD_PX = 64;

export function ChatTranscript({
  messages,
  activePermission,
  permissionBusy = false,
  activeTool,
  onPermission,
  onRetryPermission,
}: Props) {
  const pendingPermissionCount = Math.max(
    activePermission ? 1 : 0,
    messages.filter(
      (message) =>
        message.role === "permission" && message.status === "pending",
    ).length,
  );
  const scrollRef = useRef<HTMLElement>(null);
  const pinnedRef = useRef(true);
  const firstMessageIdRef = useRef<string | undefined>(messages[0]?.id);

  // Track whether the user is pinned near the bottom; scrolling up unpins.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onScroll = () => {
      const atBottom =
        el.scrollHeight - el.scrollTop - el.clientHeight <
        BOTTOM_PIN_THRESHOLD_PX;
      pinnedRef.current = atBottom;
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  // Auto-scroll on new content only while pinned; a new conversation re-pins.
  useEffect(() => {
    const firstId = messages[0]?.id;
    if (firstId !== firstMessageIdRef.current) {
      firstMessageIdRef.current = firstId;
      pinnedRef.current = true;
    }
    const el = scrollRef.current;
    if (el && pinnedRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages]);

  return (
    <section
      ref={scrollRef}
      className="chat-transcript"
      aria-label="对话记录"
      aria-live="polite"
    >
      {messages.length === 0 ? (
        <div className="chat-empty">
          <div className="chat-empty__mark" aria-hidden="true">
            <Bot size={23} />
          </div>
          <h2>准备好开始了吗？</h2>
          <p>
            描述任务、附加上下文，CC Panel 会把请求交给本地 Claude Code 会话。
          </p>
          <div className="chat-empty__tips" aria-label="快捷提示">
            <span>Enter 发送</span>
            <span>Ctrl + Enter 换行</span>
          </div>
        </div>
      ) : (
        <div className="chat-messages">
          {messages.map((message) => (
            <MessageBubble
              key={message.id}
              message={message}
              permissionBusy={
                permissionBusy ||
                (message.role === "permission" && message.status !== "pending")
              }
              onPermission={onPermission}
              onRetryPermission={onRetryPermission}
            />
          ))}
        </div>
      )}
      {activeTool &&
        activeTool.state !== "completed" &&
        activeTool.state !== "error" &&
        activeTool.state !== "cancelled" && <ActiveToolCard {...activeTool} />}
      {activePermission?.requestId &&
        !messages.some((message) => message.id === activePermission.id) && (
          <PermissionCard
            requestId={activePermission.requestId}
            toolName={activePermission.toolName}
            input={activePermission.toolInput}
            expiresAt={activePermission.permissionExpiresAt}
            pendingCount={pendingPermissionCount}
            busy={permissionBusy}
            onRespond={(behavior) =>
              onPermission(activePermission.requestId!, behavior)
            }
            onRetry={() => onRetryPermission(activePermission.requestId!)}
          />
        )}
    </section>
  );
}

const MessageBubble = memo(function MessageBubble({
  message,
  permissionBusy = false,
  onPermission,
  onRetryPermission,
}: {
  message: ChatMessage;
  permissionBusy?: boolean;
  onPermission?: (requestId: string, behavior: PermissionDecision) => void;
  onRetryPermission?: (requestId: string) => void;
}) {
  const isUser = message.role === "user";
  const displayContent = isUser
    ? extractDisplayedUserPrompt(message.content)
    : message.content;
  const isError = message.role === "error" || message.status === "error";
  return (
    <article
      className={`chat-message chat-message--${message.role} ${isError ? "chat-message--error" : ""}`}
    >
      <div className="chat-message__avatar" aria-hidden="true">
        {isUser ? <UserRound size={15} /> : <Bot size={15} />}
      </div>
      <div className="chat-message__content">
        <div className="chat-message__meta">
          <strong>
            {isUser ? "你" : message.role === "thinking" ? "思考" : "Claude"}
          </strong>
          {message.status === "running" && (
            <span className="chat-message__live">生成中</span>
          )}
        </div>
        {displayContent &&
          (message.status === "running" ? (
            <StreamingText text={displayContent} />
          ) : (
            <div className="markdown-body">
              <ReactMarkdown rehypePlugins={[rehypeHighlight]}>
                {displayContent}
              </ReactMarkdown>
            </div>
          ))}
        {message.role === "permission" && message.requestId && onPermission && (
          <PermissionCard
            requestId={message.requestId}
            toolName={message.toolName}
            input={message.toolInput}
            expiresAt={message.permissionExpiresAt}
            busy={permissionBusy}
            onRespond={(behavior) => onPermission(message.requestId!, behavior)}
            onRetry={() => onRetryPermission?.(message.requestId!)}
          />
        )}
        {message.blocks?.map((block, index) => (
          <BlockCard
            block={block}
            key={`${message.id}-${block.type}-${index}`}
          />
        ))}
      </div>
    </article>
  );
}, isSameBubble);

/**
 * Raw-text view for a message still streaming. Markdown + syntax highlighting
 * re-parse on every delta, which is O(n²) over a long reply and the main
 * source of the mid-conversation freeze — so while `status === "running"` we
 * render plain pre-wrapped text (cheap reconciliation per token) and switch to
 * the full ReactMarkdown tree once the message is committed.
 */
function StreamingText({ text }: { text: string }) {
  return <div className="markdown-body markdown-body--streaming">{text}</div>;
}

function isSameBubble(
  prev: {
    message: ChatMessage;
    permissionBusy?: boolean;
    onPermission?: (requestId: string, behavior: PermissionDecision) => void;
    onRetryPermission?: (requestId: string) => void;
  },
  next: {
    message: ChatMessage;
    permissionBusy?: boolean;
    onPermission?: (requestId: string, behavior: PermissionDecision) => void;
    onRetryPermission?: (requestId: string) => void;
  },
) {
  return (
    prev.message === next.message &&
    prev.permissionBusy === next.permissionBusy &&
    prev.onPermission === next.onPermission &&
    prev.onRetryPermission === next.onRetryPermission
  );
}
