use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{ipc::Channel, State};

use crate::{
    conversations::{ConversationStatus, UpsertConversation},
    dto::{ApiError, ApiResult, CompositionRequest, CompositionResult},
    model_profiles::ModelProfileView,
    prompt::resolve_composition,
    sessions::{
        LifecycleState, ProtocolEvent, ProtocolEventKind, ProtocolMessage, SessionError,
        SessionErrorCode, SessionEvent, SessionEventPayload, SessionMode, SessionStart, UserInput,
        PERMISSION_RESPONSE_TIMEOUT,
    },
    state::AppState,
};

const AUTOCOMPACT_TOKENS: u64 = 272_000;
const MAX_HISTORY_MESSAGE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartClaudeSessionRequest {
    pub mode: StartClaudeSessionMode,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartClaudeSessionMode {
    New,
    Resume,
    Continue,
    Fork,
    Retry,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionResponse {
    pub session_id: String,
    pub run_id: String,
    pub status: String,
    pub auto_compact_tokens: u64,
    pub compaction_observable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClaudeMessageRequest {
    pub session_id: String,
    pub run_id: String,
    pub composition: CompositionRequest,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponseRequest {
    pub session_id: String,
    pub run_id: String,
    pub request_id: String,
    pub behavior: PermissionBehavior,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    DenyInterrupt,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRuleRequest {
    pub tool_name: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

fn permission_rule(request: PermissionRuleRequest) -> ApiResult<crate::dto::PermissionRule> {
    let tool_name = request.tool_name.trim().to_owned();
    if tool_name.is_empty() || tool_name.len() > 256 {
        return Err(ApiError::new(
            "PERMISSION_RULE_INVALID",
            "工具名无效。",
            false,
        ));
    }
    let command = request
        .command
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let cwd = request
        .cwd
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if command.as_ref().is_some_and(|value| value.len() > 4096)
        || cwd.as_ref().is_some_and(|value| value.len() > 4096)
    {
        return Err(ApiError::new(
            "PERMISSION_RULE_INVALID",
            "权限匹配规则过长。",
            false,
        ));
    }
    Ok(crate::dto::PermissionRule {
        id: uuid::Uuid::new_v4().to_string(),
        tool_name,
        command,
        cwd,
    })
}

#[tauri::command]
pub fn list_permission_rules(
    state: State<'_, AppState>,
) -> ApiResult<Vec<crate::dto::PermissionRule>> {
    Ok(state.config.permission_rules())
}

#[tauri::command]
pub fn save_permission_rule(
    request: PermissionRuleRequest,
    state: State<'_, AppState>,
) -> ApiResult<Vec<crate::dto::PermissionRule>> {
    state.config.save_permission_rule(permission_rule(request)?)
}

#[tauri::command]
pub fn delete_permission_rule(
    rule_id: String,
    state: State<'_, AppState>,
) -> ApiResult<Vec<crate::dto::PermissionRule>> {
    state.config.delete_permission_rule(&rule_id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadConversationHistoryRequest {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationHistoryResponse {
    pub session_id: String,
    pub messages: Vec<ChatMessageDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<AssistantBlockDto>>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AssistantBlockDto {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        tool_use_id: String,
        tool_name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRunEnvelope {
    pub session_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub event: ClaudeRunEvent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum ClaudeRunEvent {
    Lifecycle {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Init {
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        claude_code_version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        slash_commands: Vec<String>,
    },
    Assistant {
        message_id: String,
        blocks: Vec<AssistantBlockDto>,
    },
    Stream {
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_index: Option<usize>,
        delta_type: String,
        delta: String,
    },
    ToolUse {
        tool_use_id: String,
        tool_name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    ToolProgress {
        tool_use_id: String,
        tool_name: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        subtype: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Permission {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<Value>,
        expires_at: u64,
    },
    Compaction {
        phase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pre_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        post_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    Result {
        success: bool,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        num_turns: Option<u64>,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
    Unknown {
        raw_type: String,
    },
}

#[tauri::command]
pub async fn start_claude_session(
    request: StartClaudeSessionRequest,
    channel: Channel<ClaudeRunEnvelope>,
    state: State<'_, AppState>,
) -> ApiResult<ClaudeSessionResponse> {
    let project = state
        .project_root()
        .ok_or_else(|| ApiError::new("PROJECT_ROOT_REQUIRED", "请先选择项目目录。", false))?;
    ensure_project_directory(&project)?;

    let (profile, secrets) = resolve_profile(&state, request.profile_id.as_deref())?;
    super::bootstrap::mark_claude_onboarding_complete()?;
    let mode = build_session_mode(&request)?;
    let parent_session_id = match &mode {
        SessionMode::Fork { session_id } => Some(session_id.clone()),
        _ => request.parent_session_id.clone(),
    };
    let session_id_for_metadata = match &mode {
        SessionMode::Resume { session_id } | SessionMode::Retry { session_id } => {
            Some(session_id.clone())
        }
        SessionMode::New | SessionMode::Continue | SessionMode::Fork { .. } => None,
    };
    let mut launch = crate::sessions::LaunchOptions::new(mode.clone());
    launch.cwd = Some(project.clone());
    launch.add_dirs = state
        .config
        .preferences()
        .additional_roots
        .into_iter()
        .map(|root| std::path::PathBuf::from(root.path))
        .collect();
    launch.provider_secrets = secrets;
    launch.model = profile.as_ref().map(|item| item.model_id.clone());
    launch.include_hook_events = true;
    if let SessionMode::New = mode {
        launch.session_id = None;
    }

    let handle = state
        .sessions
        .start(SessionStart::new(launch))
        .await
        .map_err(session_error)?;
    let session_id = if matches!(mode, SessionMode::Continue | SessionMode::Fork { .. }) {
        match handle.wait_for_session_id(Duration::from_secs(15)).await {
            Ok(session_id) => session_id,
            Err(error) => {
                let _ = handle.force_stop().await;
                return Err(session_error(error));
            }
        }
    } else {
        handle.session_id()
    };
    let run_id = handle.run_id().to_string();
    let title = request
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .filter(|title| title.trim() != "新对话")
        .unwrap_or("新对话")
        .to_owned();
    let metadata_session_id = session_id_for_metadata.unwrap_or_else(|| session_id.clone());
    if let Err(error) = state.conversations.upsert(UpsertConversation {
        session_id: metadata_session_id.clone(),
        active_run_id: Some(run_id.clone()),
        title,
        project_path: project.to_string_lossy().into_owned(),
        profile_id: profile.as_ref().map(|item| item.id.clone()),
        provider_name: profile.as_ref().map(|item| item.provider_name.clone()),
        model_id: profile.as_ref().map(|item| item.model_id.clone()),
        parent_session_id,
        now_ms: now_ms(),
        status: ConversationStatus::Starting,
    }) {
        let _ = handle.force_stop().await;
        return Err(error);
    }

    spawn_event_forwarder(
        handle,
        channel,
        state.sessions.clone(),
        state.conversations.clone(),
    );
    Ok(ClaudeSessionResponse {
        session_id,
        run_id,
        status: "starting".into(),
        auto_compact_tokens: AUTOCOMPACT_TOKENS,
        compaction_observable: false,
    })
}

#[tauri::command]
pub async fn send_claude_message(
    request: SendClaudeMessageRequest,
    state: State<'_, AppState>,
) -> ApiResult<CompositionResult> {
    let handle =
        state.sessions.active_handle().await.ok_or_else(|| {
            ApiError::new("SESSION_NOT_ACTIVE", "当前没有活动的 Claude 会话。", true)
        })?;
    if handle.session_id() != request.session_id || handle.run_id().to_string() != request.run_id {
        return Err(ApiError::new(
            "SESSION_NOT_ACTIVE",
            "会话已切换，请重新选择。",
            true,
        ));
    }
    let composition = resolve_composition(&request.composition, &state)?;
    handle
        .send_user(UserInput::text(composition.text.clone()))
        .await
        .map_err(session_error)?;
    Ok(composition)
}

#[tauri::command]
pub async fn stop_claude_session(
    session_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> ApiResult<()> {
    let handle =
        state.sessions.active_handle().await.ok_or_else(|| {
            ApiError::new("SESSION_NOT_ACTIVE", "当前没有活动的 Claude 会话。", true)
        })?;
    if handle.session_id() != session_id || handle.run_id().to_string() != run_id {
        return Err(ApiError::new("SESSION_NOT_ACTIVE", "会话已切换。", true));
    }
    handle.stop().await.map_err(session_error)?;
    wait_for_run_release(&state.sessions, handle.run_id(), Duration::from_secs(8)).await
}

#[tauri::command]
pub async fn respond_to_permission(
    request: PermissionResponseRequest,
    state: State<'_, AppState>,
) -> ApiResult<()> {
    let handle =
        state.sessions.active_handle().await.ok_or_else(|| {
            ApiError::new("SESSION_NOT_ACTIVE", "当前没有活动的 Claude 会话。", true)
        })?;
    if handle.session_id() != request.session_id || handle.run_id().to_string() != request.run_id {
        return Err(ApiError::new("SESSION_NOT_ACTIVE", "会话已切换。", true));
    }
    match request.behavior {
        PermissionBehavior::Allow => handle.allow(request.request_id).await,
        PermissionBehavior::Deny => {
            handle
                .deny(
                    request.request_id,
                    request
                        .message
                        .unwrap_or_else(|| "用户拒绝了此操作。".into()),
                    false,
                )
                .await
        }
        PermissionBehavior::DenyInterrupt => {
            handle
                .deny(
                    request.request_id,
                    request
                        .message
                        .unwrap_or_else(|| "用户拒绝了此操作并中断了当前回合。".into()),
                    true,
                )
                .await
        }
    }
    .map_err(session_error)
}

#[tauri::command]
pub async fn retry_permission(
    session_id: String,
    run_id: String,
    request_id: String,
    state: State<'_, AppState>,
) -> ApiResult<()> {
    let handle =
        state.sessions.active_handle().await.ok_or_else(|| {
            ApiError::new("SESSION_NOT_ACTIVE", "当前没有活动的 Claude 会话。", true)
        })?;
    if handle.session_id() != session_id || handle.run_id().to_string() != run_id {
        return Err(ApiError::new("SESSION_NOT_ACTIVE", "会话已切换。", true));
    }
    handle
        .retry_permission(request_id)
        .await
        .map_err(session_error)
}

#[tauri::command]
pub fn load_conversation_history(
    request: LoadConversationHistoryRequest,
    state: State<'_, AppState>,
) -> ApiResult<ConversationHistoryResponse> {
    let project = state
        .conversations
        .list()
        .into_iter()
        .find(|item| item.session_id == request.conversation_id)
        .ok_or_else(|| ApiError::new("CONVERSATION_NOT_FOUND", "找不到指定会话。", false))?;
    let cwd = Path::new(&project.project_path);
    let loader =
        crate::sessions::HistoryLoader::new(state.paths.home().join(".claude").join("projects"));
    let snapshot = loader
        .load(&request.conversation_id, Some(cwd))
        .map_err(|error| ApiError::new("HISTORY_UNAVAILABLE", error.to_string(), true))?;
    Ok(ConversationHistoryResponse {
        session_id: snapshot.session_id,
        messages: snapshot
            .turns
            .into_iter()
            .enumerate()
            .map(|(index, turn)| {
                history_turn_to_message(index, turn.role, turn.message_id, turn.content)
            })
            .collect(),
    })
}

fn resolve_profile(
    state: &AppState,
    requested_id: Option<&str>,
) -> ApiResult<(Option<ModelProfileView>, crate::sessions::ProviderSecrets)> {
    let profiles = state.model_profiles.list()?;
    let selected = requested_id.map(str::to_owned).or_else(|| {
        profiles
            .profiles
            .iter()
            .find(|item| item.selected)
            .map(|item| item.id.clone())
    });
    let Some(id) = selected else {
        return Ok((None, crate::sessions::ProviderSecrets::default()));
    };
    let secret = state.model_profiles.resolve_secret(&id)?;
    let profile = secret.profile.clone();
    let secrets = crate::sessions::ProviderSecrets::anthropic_compatible(
        secret.api_key().to_owned(),
        profile.base_url.clone(),
        profile.model_id.clone(),
    );
    Ok((Some(profile), secrets))
}

fn build_session_mode(request: &StartClaudeSessionRequest) -> ApiResult<SessionMode> {
    match request.mode {
        StartClaudeSessionMode::New => Ok(SessionMode::New),
        StartClaudeSessionMode::Continue => Ok(SessionMode::Continue),
        StartClaudeSessionMode::Resume => request
            .session_id
            .clone()
            .map(|session_id| SessionMode::Resume { session_id })
            .ok_or_else(|| ApiError::new("SESSION_ID_REQUIRED", "Resume 需要会话 ID。", false)),
        StartClaudeSessionMode::Retry => request
            .session_id
            .clone()
            .map(|session_id| SessionMode::Retry { session_id })
            .ok_or_else(|| ApiError::new("SESSION_ID_REQUIRED", "Retry 需要会话 ID。", false)),
        StartClaudeSessionMode::Fork => request
            .parent_session_id
            .clone()
            .or_else(|| request.session_id.clone())
            .map(|session_id| SessionMode::Fork { session_id })
            .ok_or_else(|| ApiError::new("SESSION_ID_REQUIRED", "Fork 需要父会话 ID。", false)),
    }
}

fn ensure_project_directory(path: &Path) -> ApiResult<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| ApiError::new("PROJECT_ROOT_INVALID", "项目目录不可用。", false))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ApiError::new(
            "PROJECT_ROOT_INVALID",
            "项目目录不是安全的普通目录。",
            false,
        ));
    }
    Ok(())
}

async fn wait_for_run_release(
    sessions: &crate::sessions::SessionManager,
    run_id: uuid::Uuid,
    wait: Duration,
) -> ApiResult<()> {
    let released = tokio::time::timeout(wait, async {
        loop {
            match sessions.active_handle().await {
                Some(handle) if handle.run_id() == run_id => {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                _ => return,
            }
        }
    })
    .await;
    released.map_err(|_| session_error(SessionError::StopTimeout))
}

fn spawn_event_forwarder(
    handle: crate::sessions::SessionHandle,
    channel: Channel<ClaudeRunEnvelope>,
    sessions: crate::sessions::SessionManager,
    conversations: crate::conversations::ConversationIndex,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = handle.recv().await {
            let mut envelope = normalize_session_event(&event);
            if envelope.session_id.is_empty() {
                envelope.session_id = handle.session_id();
            }
            update_conversation_status(&conversations, &envelope);
            if channel.send(envelope).is_err() {
                let _ = sessions.force_shutdown().await;
                break;
            }
        }
    });
}

fn update_conversation_status(
    index: &crate::conversations::ConversationIndex,
    envelope: &ClaudeRunEnvelope,
) {
    let status = match &envelope.event {
        ClaudeRunEvent::Lifecycle { status, .. } => match status.as_str() {
            "starting" => Some(ConversationStatus::Starting),
            "running" => Some(ConversationStatus::Running),
            "interrupted" => Some(ConversationStatus::Completed),
            "stopping" => Some(ConversationStatus::Stopping),
            "exited" => Some(ConversationStatus::Completed),
            "failed" | "timed-out" => Some(ConversationStatus::Failed),
            _ => None,
        },
        ClaudeRunEvent::Permission { .. } => Some(ConversationStatus::AwaitingPermission),
        ClaudeRunEvent::Result { success, .. } => Some(if *success {
            ConversationStatus::Completed
        } else {
            ConversationStatus::Failed
        }),
        ClaudeRunEvent::Error { .. } => Some(ConversationStatus::Failed),
        _ => None,
    };
    if let Some(status) = status {
        let _ = index.set_status(&envelope.session_id, &envelope.run_id, status, now_ms());
    }
}

fn normalize_session_event(event: &SessionEvent) -> ClaudeRunEnvelope {
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
        tool_name: request.and_then(|item| string_at(&Value::Object(item.clone()), &["tool_name"])),
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

fn history_turn_to_message(
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

fn session_error(error: SessionError) -> ApiError {
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

fn now_ms() -> u64 {
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
