use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const MAX_STDIN_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUEST_ID_BYTES: usize = 256;

/// A user turn accepted by Claude's bidirectional `stream-json` input.
///
/// `content` is deliberately restricted to the two forms understood by the
/// official CLI: a string or an array of content blocks. The value is always
/// serialized as JSON; it is never interpolated into a command line.
#[derive(Debug, Clone, PartialEq)]
pub struct UserInput {
    content: Value,
}

impl UserInput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: Value::String(text.into()),
        }
    }

    pub fn content_blocks(blocks: Vec<Value>) -> Self {
        Self {
            content: Value::Array(blocks),
        }
    }

    pub fn content(&self) -> &Value {
        &self.content
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEventKind {
    Init,
    Assistant,
    User,
    StreamEvent,
    Result,
    PermissionRequest,
    ControlRequest,
    ControlResponse,
    ToolProgress,
    ToolUseSummary,
    ToolResult,
    CompactBoundary,
    HookStarted,
    HookProgress,
    HookResponse,
    System,
    Unknown,
}

/// Lossless, forward-compatible representation of one valid CLI NDJSON event.
///
/// Stable routing fields are lifted out for consumers while `raw` preserves
/// fields added by future Claude CLI versions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolEvent {
    pub kind: ProtocolEventKind,
    pub event_type: Option<String>,
    pub subtype: Option<String>,
    pub session_id: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MalformedReason {
    EmptyLine,
    InvalidUtf8,
    InvalidJson,
    NonObject,
}

/// A malformed record intentionally carries no source preview. A CLI error can
/// contain credentials, prompts, or tool inputs, so echoing it into logs would
/// be unsafe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MalformedProtocolLine {
    pub byte_len: usize,
    pub reason: MalformedReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ProtocolMessage {
    Event(ProtocolEvent),
    Malformed(MalformedProtocolLine),
}

impl ProtocolMessage {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Event(event) => event.session_id.as_deref(),
            Self::Malformed(_) => None,
        }
    }

    pub fn permission_request(&self) -> Option<PermissionRequest> {
        let Self::Event(event) = self else {
            return None;
        };
        if event.kind != ProtocolEventKind::PermissionRequest {
            return None;
        }
        PermissionRequest::from_event(event)
    }
}

/// The subset of `control_request/can_use_tool` needed to safely complete a
/// permission round trip. `input` is retained verbatim and is the only value
/// used as `updatedInput` when permission is granted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_name: String,
    pub input: Value,
    pub tool_use_id: Option<String>,
    pub permission_suggestions: Option<Value>,
}

impl PermissionRequest {
    fn from_event(event: &ProtocolEvent) -> Option<Self> {
        let object = event.raw.as_object()?;
        let request_id = object.get("request_id")?.as_str()?.to_owned();
        if !valid_request_id(&request_id) {
            return None;
        }
        let request = object.get("request")?.as_object()?;
        if request.get("subtype")?.as_str()? != "can_use_tool" {
            return None;
        }
        let tool_name = request.get("tool_name")?.as_str()?.to_owned();
        let input = request.get("input")?.clone();
        Some(Self {
            request_id,
            tool_name,
            input,
            tool_use_id: request
                .get("tool_use_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            permission_suggestions: request.get("permission_suggestions").cloned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    /// The manager returns the request's original input as `updatedInput`.
    Allow,
    Deny {
        message: String,
        /// Ask Claude to stop the active turn after recording the denial.
        interrupt: bool,
    },
}

#[derive(Debug, Error)]
pub enum ProtocolEncodeError {
    #[error("the protocol request id is invalid")]
    InvalidRequestId,
    #[error("the user content must be a string or an array of content blocks")]
    InvalidUserContent,
    #[error("the encoded stdin message exceeds the safety limit")]
    MessageTooLarge,
    #[error("failed to encode a Claude CLI protocol message")]
    Json(#[source] serde_json::Error),
}

pub fn parse_protocol_line(line: &[u8]) -> ProtocolMessage {
    let line = strip_line_ending(line);
    if line.is_empty() {
        return ProtocolMessage::Malformed(MalformedProtocolLine {
            byte_len: 0,
            reason: MalformedReason::EmptyLine,
        });
    }
    if std::str::from_utf8(line).is_err() {
        return ProtocolMessage::Malformed(MalformedProtocolLine {
            byte_len: line.len(),
            reason: MalformedReason::InvalidUtf8,
        });
    }
    let value: Value = match serde_json::from_slice(line) {
        Ok(value) => value,
        Err(_) => {
            return ProtocolMessage::Malformed(MalformedProtocolLine {
                byte_len: line.len(),
                reason: MalformedReason::InvalidJson,
            });
        }
    };
    let Some(object) = value.as_object() else {
        return ProtocolMessage::Malformed(MalformedProtocolLine {
            byte_len: line.len(),
            reason: MalformedReason::NonObject,
        });
    };

    let event_type = object
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let subtype = object
        .get("subtype")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let session_id = object
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let kind = classify_event(event_type.as_deref(), subtype.as_deref(), object);

    ProtocolMessage::Event(ProtocolEvent {
        kind,
        event_type,
        subtype,
        session_id,
        raw: value,
    })
}

pub(crate) fn encode_user_input(input: &UserInput) -> Result<Vec<u8>, ProtocolEncodeError> {
    if !matches!(input.content, Value::String(_) | Value::Array(_)) {
        return Err(ProtocolEncodeError::InvalidUserContent);
    }
    encode_line(&json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": input.content,
        }
    }))
}

pub(crate) fn encode_permission_response(
    request: &PermissionRequest,
    decision: &PermissionDecision,
) -> Result<Vec<u8>, ProtocolEncodeError> {
    if !valid_request_id(&request.request_id) {
        return Err(ProtocolEncodeError::InvalidRequestId);
    }

    let decision = match decision {
        PermissionDecision::Allow => json!({
            "behavior": "allow",
            // The current Agent SDK contract requires this field even when the
            // host did not modify the tool input.
            "updatedInput": request.input.clone(),
        }),
        PermissionDecision::Deny { message, interrupt } => {
            let mut value = json!({
                "behavior": "deny",
                "message": message,
            });
            if *interrupt {
                value["interrupt"] = Value::Bool(true);
            }
            value
        }
    };

    encode_line(&json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request.request_id,
            "response": decision,
        }
    }))
}

pub(crate) fn encode_interrupt(request_id: &str) -> Result<Vec<u8>, ProtocolEncodeError> {
    if !valid_request_id(request_id) {
        return Err(ProtocolEncodeError::InvalidRequestId);
    }
    encode_line(&json!({
        "type": "control_request",
        "request_id": request_id,
        "request": {
            "subtype": "interrupt",
        }
    }))
}

fn encode_line(value: &Value) -> Result<Vec<u8>, ProtocolEncodeError> {
    let mut encoded = serde_json::to_vec(value).map_err(ProtocolEncodeError::Json)?;
    encoded.push(b'\n');
    if encoded.len() > MAX_STDIN_MESSAGE_BYTES {
        return Err(ProtocolEncodeError::MessageTooLarge);
    }
    Ok(encoded)
}

fn valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= MAX_REQUEST_ID_BYTES
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn classify_event(
    event_type: Option<&str>,
    subtype: Option<&str>,
    object: &serde_json::Map<String, Value>,
) -> ProtocolEventKind {
    match (event_type, subtype) {
        (Some("system"), Some("init")) => ProtocolEventKind::Init,
        (Some("system"), Some("compact_boundary")) => ProtocolEventKind::CompactBoundary,
        (Some("system"), Some("hook_started")) => ProtocolEventKind::HookStarted,
        (Some("system"), Some("hook_progress")) => ProtocolEventKind::HookProgress,
        (Some("system"), Some("hook_response")) => ProtocolEventKind::HookResponse,
        (Some("system"), _) => ProtocolEventKind::System,
        (Some("assistant"), _) => ProtocolEventKind::Assistant,
        (Some("user"), _) => ProtocolEventKind::User,
        (Some("stream_event"), _) => ProtocolEventKind::StreamEvent,
        (Some("result"), _) => ProtocolEventKind::Result,
        (Some("control_request"), _) => {
            let is_permission = object
                .get("request")
                .and_then(Value::as_object)
                .and_then(|request| request.get("subtype"))
                .and_then(Value::as_str)
                == Some("can_use_tool");
            if is_permission {
                ProtocolEventKind::PermissionRequest
            } else {
                ProtocolEventKind::ControlRequest
            }
        }
        (Some("control_response"), _) => ProtocolEventKind::ControlResponse,
        (Some("tool_progress"), _) => ProtocolEventKind::ToolProgress,
        (Some("tool_use_summary"), _) => ProtocolEventKind::ToolUseSummary,
        (Some("tool_result"), _) => ProtocolEventKind::ToolResult,
        _ => ProtocolEventKind::Unknown,
    }
}

fn strip_line_ending(mut line: &[u8]) -> &[u8] {
    if line.last() == Some(&b'\n') {
        line = &line[..line.len() - 1];
    }
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encodes_user_text_as_ndjson_without_interpolation() {
        let encoded = encode_user_input(&UserInput::text("line 1\n\"quoted\"; --resume bad"))
            .expect("user input should encode");
        assert!(encoded.ends_with(b"\n"));
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            value,
            json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": "line 1\n\"quoted\"; --resume bad"
                }
            })
        );
    }

    #[test]
    fn allow_uses_the_exact_original_input() {
        let original = json!({"command": "printf '%s' \"$TOKEN\"", "nested": {"n": 1}});
        let request = PermissionRequest {
            request_id: "req_123".into(),
            tool_name: "Bash".into(),
            input: original.clone(),
            tool_use_id: Some("toolu_1".into()),
            permission_suggestions: None,
        };
        let encoded = encode_permission_response(&request, &PermissionDecision::Allow).unwrap();
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["response"]["response"]["updatedInput"], original);
        assert_eq!(value["response"]["response"]["behavior"], "allow");
    }

    #[test]
    fn deny_can_interrupt_the_turn() {
        let request = PermissionRequest {
            request_id: "req-1".into(),
            tool_name: "Write".into(),
            input: json!({"path": "x"}),
            tool_use_id: None,
            permission_suggestions: None,
        };
        let encoded = encode_permission_response(
            &request,
            &PermissionDecision::Deny {
                message: "not approved".into(),
                interrupt: true,
            },
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["response"]["subtype"], "success");
        assert_eq!(value["response"]["response"]["behavior"], "deny");
        assert_eq!(value["response"]["response"]["interrupt"], true);
    }

    #[test]
    fn parses_permission_and_preserves_unknown_fields() {
        let source = br#"{"type":"control_request","request_id":"req_7","request":{"subtype":"can_use_tool","tool_name":"Edit","input":{"x":1},"permission_suggestions":[{"type":"addRules"}],"future":true},"top_future":42}"#;
        let parsed = parse_protocol_line(source);
        let permission = parsed.permission_request().unwrap();
        assert_eq!(permission.request_id, "req_7");
        assert_eq!(permission.tool_name, "Edit");
        assert_eq!(permission.input, json!({"x": 1}));
        let ProtocolMessage::Event(event) = parsed else {
            panic!("expected valid event")
        };
        assert_eq!(event.kind, ProtocolEventKind::PermissionRequest);
        assert_eq!(event.raw["top_future"], 42);
    }

    #[test]
    fn classifies_current_stream_event_families() {
        let cases = [
            (
                r#"{"type":"system","subtype":"init","session_id":"s"}"#,
                ProtocolEventKind::Init,
            ),
            (
                r#"{"type":"assistant","message":{}}"#,
                ProtocolEventKind::Assistant,
            ),
            (
                r#"{"type":"stream_event","event":{}}"#,
                ProtocolEventKind::StreamEvent,
            ),
            (
                r#"{"type":"result","subtype":"success"}"#,
                ProtocolEventKind::Result,
            ),
            (
                r#"{"type":"tool_progress"}"#,
                ProtocolEventKind::ToolProgress,
            ),
            (
                r#"{"type":"tool_use_summary"}"#,
                ProtocolEventKind::ToolUseSummary,
            ),
            (
                r#"{"type":"system","subtype":"compact_boundary"}"#,
                ProtocolEventKind::CompactBoundary,
            ),
            (
                r#"{"type":"system","subtype":"hook_started"}"#,
                ProtocolEventKind::HookStarted,
            ),
            (
                r#"{"type":"future_event","new_field":true}"#,
                ProtocolEventKind::Unknown,
            ),
        ];
        for (source, expected) in cases {
            let ProtocolMessage::Event(event) = parse_protocol_line(source.as_bytes()) else {
                panic!("expected valid event")
            };
            assert_eq!(event.kind, expected, "source: {source}");
        }
    }

    #[test]
    fn malformed_data_never_echoes_source_content() {
        let parsed = parse_protocol_line(b"secret=sk-ant-not-json");
        assert_eq!(
            parsed,
            ProtocolMessage::Malformed(MalformedProtocolLine {
                byte_len: 22,
                reason: MalformedReason::InvalidJson,
            })
        );
    }

    #[test]
    fn interrupt_is_a_control_request() {
        let value: Value =
            serde_json::from_slice(&encode_interrupt("interrupt_1").unwrap()).unwrap();
        assert_eq!(value["type"], "control_request");
        assert_eq!(value["request_id"], "interrupt_1");
        assert_eq!(value["request"]["subtype"], "interrupt");
    }
}
