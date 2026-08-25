use std::{path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{ipc::Channel, State};

use super::session_events::{
    history_turn_to_message, normalize_session_event, now_ms, session_error,
};
use crate::{
    conversations::{ConversationStatus, UpsertConversation},
    dto::{ApiError, ApiResult, CompositionRequest, CompositionResult},
    model_profiles::ModelProfileView,
    prompt::resolve_composition,
    sessions::{SessionError, SessionMode, SessionStart, UserInput},
    state::AppState,
};

const AUTOCOMPACT_TOKENS: u64 = 272_000;

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
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
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
