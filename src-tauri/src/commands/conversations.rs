use tauri::State;

use crate::{
    conversations::ConversationMetadata,
    dto::{ApiError, ApiResult},
    state::AppState,
};

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Vec<ConversationMetadata> {
    state.conversations.list()
}

#[tauri::command]
pub fn delete_conversation(
    session_id: String,
    state: State<'_, AppState>,
) -> ApiResult<Vec<ConversationMetadata>> {
    state
        .conversations
        .delete(&session_id)
        .map_err(|_| ApiError::new("CONVERSATION_NOT_FOUND", "找不到指定会话。", false))?;
    Ok(state.conversations.list())
}

#[tauri::command]
pub fn rename_conversation(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> ApiResult<Vec<ConversationMetadata>> {
    state.conversations.rename(&session_id, &title)?;
    Ok(state.conversations.list())
}

#[tauri::command]
pub fn set_conversation_favorite(
    session_id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> ApiResult<Vec<ConversationMetadata>> {
    state.conversations.set_favorite(&session_id, favorite)?;
    Ok(state.conversations.list())
}

#[tauri::command]
pub fn set_conversation_archived(
    session_id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> ApiResult<Vec<ConversationMetadata>> {
    state.conversations.set_archived(&session_id, archived)?;
    Ok(state.conversations.list())
}
