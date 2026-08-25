//! Internal conversion layer between Claude CLI session events and frontend DTOs.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::{AssistantBlockDto, ChatMessageDto, ClaudeRunEnvelope, ClaudeRunEvent};
use crate::{
    dto::ApiError,
    sessions::{
        LifecycleState, ProtocolEvent, ProtocolEventKind, ProtocolMessage, SessionError,
        SessionErrorCode, SessionEvent, SessionEventPayload, PERMISSION_RESPONSE_TIMEOUT,
    },
};

const MAX_HISTORY_MESSAGE_BYTES: usize = 256 * 1024;

pub(super) fn normalize_session_event(event: &SessionEvent) -> ClaudeRunEnvelope {
    let normalized = match &event.payload {
        SessionEventPayload::Lifecycle { state } => ClaudeRunEvent::Lifecycle {
            status: lifecycle_name(state).into(),
            message: None,
        },
        SessionEventPayload::Protocol { message } => normalize_protocol(message),
        SessionEventPayload::Stderr { .. } => ClaudeRunEvent::Unknown {
            raw_type: "stderr-diagnostic".into(),
        },
        SessionEventPayload::WatchdogTimeout => ClaudeRunEvent::Error {
            code: "WATCHDOG_TIMEOUT".into(),
            message: "Claude 会话超过 30 分钟未完成，已停止。".into(),
            retryable: true,
        },
        SessionEventPayload::Exited { code } => ClaudeRunEvent::Lifecycle {
            status: if code.is_some_and(|value| value == 0) {
                "exited"
            } else {
                "failed"
            }
            .into(),
            message: None,
        },
        SessionEventPayload::Error { code, retryable } => ClaudeRunEvent::Error {
            code: error_code(code).into(),
            message: error_message(code).into(),
            retryable: *retryable,
        },
    };
    ClaudeRunEnvelope {
        session_id: event.session_id.clone(),
        run_id: event.run_id.to_string(),
        sequence: event.sequence,
        event: normalized,
    }
}

fn normalize_protocol(message: &ProtocolMessage) -> ClaudeRunEvent {
    let ProtocolMessage::Event(event) = message else {
        return ClaudeRunEvent::Unknown {
            raw_type: "malformed".into(),
        };
    };
    match event.kind {
        ProtocolEventKind::Init => normalize_init(event),
        ProtocolEventKind::Assistant => normalize_assistant(event),
        ProtocolEventKind::StreamEvent => normalize_stream(event),
        ProtocolEventKind::PermissionRequest => normalize_permission(event),
        ProtocolEventKind::ToolResult => normalize_tool_result(event),
        ProtocolEventKind::ToolProgress | ProtocolEventKind::ToolUseSummary => {
            normalize_tool_progress(event)
        }
        ProtocolEventKind::CompactBoundary => normalize_compaction(event, "completed"),
        ProtocolEventKind::HookStarted if is_precompact_hook(event) => {
            normalize_compaction(event, "starting")
        }
        ProtocolEventKind::Result => normalize_result(event),
        ProtocolEventKind::Unknown => ClaudeRunEvent::Unknown {
            raw_type: event.event_type.clone().unwrap_or_else(|| "unknown".into()),
        },
        _ => ClaudeRunEvent::Unknown {
            raw_type: event
                .event_type
                .clone()
                .unwrap_or_else(|| "unsupported".into()),
        },
    }
}

fn normalize_init(event: &ProtocolEvent) -> ClaudeRunEvent {
    ClaudeRunEvent::Init {
        model: string_at(&event.raw, &["model", "model_id"]),
        claude_code_version: string_at(&event.raw, &["claude_code_version", "version"]),
        permission_mode: string_at(&event.raw, &["permissionMode", "permission_mode"]),
        slash_commands: string_array_at(&event.raw, &["slash_commands", "slashCommands"]),
    }
}

fn normalize_assistant(event: &ProtocolEvent) -> ClaudeRunEvent {
    let message = event.raw.get("message").unwrap_or(&Value::Null);
    let message_id = string_at(message, &["id", "message_id"]).unwrap_or_else(|| {
        format!(
            "assistant-{}",
            event.session_id.as_deref().unwrap_or("message")
        )
    });
    let blocks = message
        .get("content")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(normalize_block).collect())
        .unwrap_or_default();
    ClaudeRunEvent::Assistant { message_id, blocks }
}

fn normalize_stream(event: &ProtocolEvent) -> ClaudeRunEvent {
    let source = event.raw.get("event").unwrap_or(&event.raw);
    let delta = source.get("delta").unwrap_or(&Value::Null);
    let delta_type = string_at(delta, &["type"]).unwrap_or_else(|| "text_delta".into());
    let kind = match delta_type.as_str() {
        "thinking_delta" => "thinking",
        "input_json_delta" => "input-json",
        _ => "text",
    };
    ClaudeRunEvent::Stream {
        message_id: string_at(&event.raw, &["message_id"])
            .or_else(|| string_at(source, &["message_id"])),
        block_index: source
            .get("index")
            .and_then(Value::as_u64)
            .map(|v| v as usize),
        delta_type: kind.into(),
        delta: string_at(delta, &["text", "thinking", "partial_json"]).unwrap_or_default(),
    }
}

fn normalize_permission(event: &ProtocolEvent) -> ClaudeRunEvent {
    let request_id = string_at(&event.raw, &["request_id"]);
    let request = event.raw.get("request").and_then(Value::as_object);
    let valid = request_id
        .as_deref()
        .is_some_and(|value| !value.is_empty() && value.len() <= 256)
        && request.is_some_and(|item| {
            item.get("subtype").and_then(Value::as_str) == Some("can_use_tool")
                && item.get("tool_name").and_then(Value::as_str).is_some()
                && item.get("input").is_some()
        });
    if !valid {
        return ClaudeRunEvent::Unknown {
            raw_type: "malformed-permission-request".into(),
        };
    }
    ClaudeRunEvent::Permission {
        request_id: request_id.expect("validated request id"),
        tool_use_id: request
            .and_then(|item| string_at(&Value::Object(item.clone()), &["tool_use_id"])),
        tool_name: request.and_then(|item| {
            item.get("tool_name")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }),
        input: request.and_then(|item| item.get("input").cloned()),
        expires_at: now_ms() + PERMISSION_RESPONSE_TIMEOUT.as_millis() as u64,
    }
}

fn normalize_tool_result(event: &ProtocolEvent) -> ClaudeRunEvent {
    let tool_use_id = string_at(&event.raw, &["tool_use_id"]).unwrap_or_default();
    let content = bounded_display(event.raw.get("content"));
    ClaudeRunEvent::ToolResult {
        tool_use_id,
        content,
        is_error: event
            .raw
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Surfaces live tool execution to the transcript. Both `tool_progress`
/// (streaming stdout/stderr and state transitions) and `tool_use_summary`
/// (completion timing) are folded into one `ToolProgress` envelope so the
/// frontend can show "Bash 正在运行" and its live output while the tool runs,
/// instead of appearing frozen until the `tool_result` lands.
fn normalize_tool_progress(event: &ProtocolEvent) -> ClaudeRunEvent {
    let is_summary = event.kind == ProtocolEventKind::ToolUseSummary;
    let tool_use_id = string_at(&event.raw, &["tool_use_id"]).unwrap_or_default();
    let tool_name = string_at(&event.raw, &["tool_name"]).unwrap_or_default();
    let state = if is_summary {
        "completed".to_owned()
    } else {
        string_at(&event.raw, &["state"])
            .filter(|state| !state.is_empty())
            .unwrap_or_else(|| "in_progress".to_owned())
    };
    let subtype = if is_summary {
        Some("summary".to_owned())
    } else {
        string_at(&event.raw, &["subtype"])
    };
    let text = if is_summary {
        event
            .raw
            .get("duration_ms")
            .or_else(|| event.raw.get("durationMs"))
            .and_then(Value::as_u64)
            .map(|millis| format!("{:.1}s", millis as f64 / 1000.0))
    } else {
        string_at(&event.raw, &["text"]).or_else(|| string_at(&event.raw, &["output"]))
    };
    ClaudeRunEvent::ToolProgress {
        tool_use_id,
        tool_name,
        state,
        subtype,
        text,
    }
}

fn is_precompact_hook(event: &ProtocolEvent) -> bool {
    [
        event.raw.get("hook_name"),
        event.raw.get("hookName"),
        event.raw.get("hook_event_name"),
        event.raw.get("hookEventName"),
        event.raw.get("hook").and_then(|hook| hook.get("name")),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .any(|name| name.eq_ignore_ascii_case("PreCompact"))
}

fn normalize_compaction(event: &ProtocolEvent, phase: &str) -> ClaudeRunEvent {
    ClaudeRunEvent::Compaction {
        phase: phase.into(),
        trigger: string_at(&event.raw, &["trigger"]),
        pre_tokens: number_at(&event.raw, &["pre_tokens", "preTokens"]),
        post_tokens: number_at(&event.raw, &["post_tokens", "postTokens"]),
        duration_ms: number_at(&event.raw, &["duration_ms", "durationMs"]),
    }
}

fn normalize_result(event: &ProtocolEvent) -> ClaudeRunEvent {
    let success = event
        .subtype
        .as_deref()
        .map(|subtype| subtype == "success")
        .unwrap_or_else(|| {
            !event
                .raw
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    ClaudeRunEvent::Result {
        success,
        is_error: !success,
        stop_reason: string_at(&event.raw, &["stop_reason", "stopReason"]),
        duration_ms: number_at(&event.raw, &["duration_ms", "durationMs"]),
        num_turns: number_at(&event.raw, &["num_turns", "numTurns"]),
    }
}

fn normalize_block(value: &Value) -> Option<AssistantBlockDto> {
    let kind = value.get("type")?.as_str()?;
    match kind {
        "text" => Some(AssistantBlockDto::Text {
            text: bounded_display(value.get("text")),
        }),
        "thinking" => Some(AssistantBlockDto::Thinking {
            thinking: bounded_display(value.get("thinking").or_else(|| value.get("text"))),
        }),
        "tool_use" => Some(AssistantBlockDto::ToolUse {
            tool_use_id: string_at(value, &["id", "tool_use_id"])?,
            tool_name: string_at(value, &["name", "tool_name"])?,
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "tool_result" => Some(AssistantBlockDto::ToolResult {
            tool_use_id: string_at(value, &["tool_use_id"])?,
            content: bounded_display(value.get("content")),
            is_error: value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        _ => None,
    }
}

pub(super) fn history_turn_to_message(
    index: usize,
    role: String,
    message_id: Option<String>,
    content: Value,
) -> ChatMessageDto {
    let id = message_id.unwrap_or_else(|| format!("history-{index}"));
    let blocks = content
        .as_array()
        .map(|items| items.iter().filter_map(normalize_block).collect::<Vec<_>>());
    let text = blocks
        .as_ref()
        .map(|items| {
            items
                .iter()
                .filter_map(|block| match block {
                    AssistantBlockDto::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        })
        .unwrap_or_else(|| bounded_display(Some(&content)));
    ChatMessageDto {
        id,
        role,
        content: text,
        blocks,
        status: "complete".into(),
    }
}

fn string_at(value: &Value, fields: &[&str]) -> Option<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str).map(str::to_owned))
}

fn string_array_at(value: &Value, fields: &[&str]) -> Vec<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .take(256)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn number_at(value: &Value, fields: &[&str]) -> Option<u64> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_u64))
}

fn bounded_display(value: Option<&Value>) -> String {
    let mut text = match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(value) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "[无法显示此数据]".into())
        }
    };
    if text.len() > MAX_HISTORY_MESSAGE_BYTES {
        let boundary = floor_char_boundary(&text, MAX_HISTORY_MESSAGE_BYTES);
        text.truncate(boundary);
        text.push_str("\n[内容已截断]");
    }
    text
}

fn floor_char_boundary(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn lifecycle_name(state: &LifecycleState) -> &'static str {
    match state {
        LifecycleState::Starting => "starting",
        LifecycleState::Running => "running",
        LifecycleState::Interrupted => "interrupted",
        LifecycleState::Stopping => "stopping",
        LifecycleState::Finished => "exited",
    }
}

fn error_code(code: &SessionErrorCode) -> &'static str {
    match code {
        SessionErrorCode::SpawnFailed => "SPAWN_FAILED",
        SessionErrorCode::OutputTooLarge => "OUTPUT_TOO_LARGE",
        SessionErrorCode::StdinClosed => "STDIN_CLOSED",
        SessionErrorCode::ProtocolWriteFailed => "PROTOCOL_WRITE_FAILED",
        SessionErrorCode::PermissionNotPending => "PERMISSION_NOT_PENDING",
        SessionErrorCode::PermissionExpired => "PERMISSION_EXPIRED",
        SessionErrorCode::WatchdogTimeout => "WATCHDOG_TIMEOUT",
        SessionErrorCode::ProcessCheckFailed => "PROCESS_CHECK_FAILED",
        SessionErrorCode::ChildExited => "CHILD_EXITED",
        SessionErrorCode::EventConsumerGone => "EVENT_CONSUMER_GONE",
        SessionErrorCode::JobObjectFailed => "JOB_OBJECT_FAILED",
    }
}

fn error_message(code: &SessionErrorCode) -> &'static str {
    match code {
        SessionErrorCode::OutputTooLarge => "Claude 输出超过安全上限。",
        SessionErrorCode::PermissionNotPending => "权限请求已过期或已被处理。",
        SessionErrorCode::PermissionExpired => "权限确认已失效，请点击重试后再处理。",
        SessionErrorCode::WatchdogTimeout => "Claude 会话超过 30 分钟未完成。",
        SessionErrorCode::ProtocolWriteFailed => "无法向 Claude 会话写入请求。",
        _ => "Claude 会话发生错误。",
    }
}

pub(super) fn session_error(error: SessionError) -> ApiError {
    match error {
        SessionError::AlreadyActive => ApiError::new(
            "SESSION_ALREADY_ACTIVE",
            "已有一个 Claude 会话正在运行。",
            true,
        ),
        SessionError::Finished => ApiError::new("SESSION_NOT_ACTIVE", "Claude 会话已结束。", true),
        SessionError::PermissionNotPending => {
            ApiError::new("PERMISSION_NOT_PENDING", "权限请求已过期或已被处理。", true)
        }
        SessionError::PermissionExpired => ApiError::new(
            "PERMISSION_EXPIRED",
            "权限确认已失效，请点击重试后再处理。",
            true,
        ),
        SessionError::PermissionRequestMismatch => {
            ApiError::new("PERMISSION_REQUEST_MISMATCH", "权限请求不匹配。", false)
        }
        SessionError::Launch(_) => {
            ApiError::new("CLAUDE_LAUNCH_FAILED", "无法启动 Claude CLI。", true)
        }
        SessionError::InvalidWatchdog => ApiError::new(
            "SESSION_CONFIGURATION_INVALID",
            "会话 watchdog 配置无效。",
            false,
        ),
        SessionError::CommandChannelClosed | SessionError::EventChannelClosed => {
            ApiError::new("SESSION_CHANNEL_CLOSED", "Claude 会话通道已关闭。", true)
        }
        SessionError::CommandTimeout => ApiError::new(
            "SESSION_COMMAND_TIMEOUT",
            "Claude 会话响应超时，请重试。",
            true,
        ),
        SessionError::IdentityTimeout => ApiError::new(
            "SESSION_ID_TIMEOUT",
            "Claude CLI 未及时返回会话标识，启动已取消。",
            true,
        ),
        SessionError::IdentityUnavailable => ApiError::new(
            "SESSION_ID_UNAVAILABLE",
            "Claude CLI 在返回会话标识前已退出。",
            true,
        ),
        SessionError::StopTimeout => ApiError::new(
            "SESSION_STOP_TIMEOUT",
            "Claude 会话未在清理期限内停止。",
            true,
        ),
        SessionError::ProtocolEncode => ApiError::new(
            "PROTOCOL_ENCODE_FAILED",
            "无法编码 Claude 会话请求。",
            false,
        ),
        SessionError::ProtocolWriteFailed => ApiError::new(
            "PROTOCOL_WRITE_FAILED",
            "无法向 Claude 会话写入请求。",
            true,
        ),
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_display_truncates_on_a_utf8_boundary() {
        let prefix = "a".repeat(MAX_HISTORY_MESSAGE_BYTES - 1);
        let value = Value::String(format!("{prefix}好"));
        let display = bounded_display(Some(&value));

        assert!(display.is_char_boundary(display.len()));
        assert!(display.ends_with("\n[内容已截断]"));
        assert_eq!(display.matches('好').count(), 0);
    }

    #[test]
    fn tool_progress_is_normalized_to_a_live_tool_event() {
        let message = ProtocolMessage::Event(ProtocolEvent {
            kind: ProtocolEventKind::ToolProgress,
            event_type: Some("tool_progress".into()),
            subtype: None,
            session_id: None,
            raw: serde_json::json!({
                "type": "tool_progress",
                "tool_use_id": "toolu_1",
                "tool_name": "Bash",
                "state": "in_progress",
                "subtype": "stdout",
                "text": "compile ok",
            }),
        });
        let envelope = normalize_protocol(&message);
        match envelope {
            ClaudeRunEvent::ToolProgress {
                tool_use_id,
                tool_name,
                state,
                subtype,
                text,
            } => {
                assert_eq!(tool_use_id, "toolu_1");
                assert_eq!(tool_name, "Bash");
                assert_eq!(state, "in_progress");
                assert_eq!(subtype.as_deref(), Some("stdout"));
                assert_eq!(text.as_deref(), Some("compile ok"));
            }
            other => panic!("expected tool-progress, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_summary_is_folded_into_a_completed_tool_event() {
        let message = ProtocolMessage::Event(ProtocolEvent {
            kind: ProtocolEventKind::ToolUseSummary,
            event_type: Some("tool_use_summary".into()),
            subtype: None,
            session_id: None,
            raw: serde_json::json!({
                "type": "tool_use_summary",
                "tool_use_id": "toolu_2",
                "tool_name": "Edit",
                "duration_ms": 2400,
            }),
        });
        let envelope = normalize_protocol(&message);
        match envelope {
            ClaudeRunEvent::ToolProgress {
                tool_use_id,
                tool_name,
                state,
                subtype,
                text,
            } => {
                assert_eq!(tool_use_id, "toolu_2");
                assert_eq!(tool_name, "Edit");
                assert_eq!(state, "completed");
                assert_eq!(subtype.as_deref(), Some("summary"));
                assert_eq!(text.as_deref(), Some("2.4s"));
            }
            other => panic!("expected tool-progress, got {other:?}"),
        }
    }
}
