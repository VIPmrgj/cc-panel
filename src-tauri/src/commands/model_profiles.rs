use tauri::State;

use crate::{
    dto::{ApiError, ApiResult, ModelConnectionTestResult},
    model_profiles::{prompt_api_key, test_connection, ModelProfileInput, ModelProfilesView},
    state::AppState,
};

#[tauri::command]
pub fn list_model_profiles(state: State<'_, AppState>) -> ApiResult<ModelProfilesView> {
    state.model_profiles.list()
}
#[tauri::command]
pub async fn test_model_profile_connection(
    profile_id: String,
    state: State<'_, AppState>,
) -> ApiResult<ModelConnectionTestResult> {
    let secret = state.model_profiles.resolve_secret(&profile_id)?;
    test_connection(secret).await
}

#[tauri::command]
pub fn save_model_profile(
    profile: ModelProfileInput,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> ApiResult<ModelProfilesView> {
    state.model_profiles.update(profile, expected_revision)
}

#[tauri::command]
pub async fn prompt_and_save_model_profile(
    profile: ModelProfileInput,
    expected_revision: u64,
    window: tauri::Window,
    state: State<'_, AppState>,
) -> ApiResult<Option<ModelProfilesView>> {
    let provider_name = profile.provider_name.clone();
    #[cfg(windows)]
    let parent_window = Some(
        window
            .hwnd()
            .map_err(|_| {
                ApiError::new(
                    "NATIVE_CREDENTIAL_PROMPT_FAILED",
                    "无法获取系统凭据窗口的父窗口。",
                    true,
                )
            })?
            .0 as usize,
    );
    #[cfg(not(windows))]
    let parent_window = {
        let _ = window;
        None
    };
    let prompted =
        tokio::task::spawn_blocking(move || prompt_api_key(&provider_name, parent_window))
            .await
            .map_err(|_| {
                ApiError::new(
                    "NATIVE_CREDENTIAL_PROMPT_FAILED",
                    "系统凭据输入窗口异常结束。",
                    true,
                )
            })??;
    let Some(api_key) = prompted else {
        return Ok(None);
    };
    state
        .model_profiles
        .save_with_api_key(profile, api_key, expected_revision)
        .map(Some)
}

/// Saves a key supplied explicitly by the onboarding password field. The key
/// is passed only to the protected storage path and is never logged or
/// returned to the renderer.
#[tauri::command]
pub fn save_model_profile_with_api_key(
    profile: ModelProfileInput,
    api_key: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> ApiResult<ModelProfilesView> {
    state
        .model_profiles
        .save_with_api_key(profile, api_key, expected_revision)
}

#[tauri::command]
pub fn delete_model_profile(
    profile_id: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> ApiResult<ModelProfilesView> {
    state.model_profiles.delete(&profile_id, expected_revision)
}

#[tauri::command]
pub fn select_model_profile(
    profile_id: Option<String>,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> ApiResult<ModelProfilesView> {
    select_profile(&state, profile_id.as_deref(), expected_revision)
}

#[tauri::command]
pub fn restore_model_profile_selection(
    profile_id: Option<String>,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> ApiResult<ModelProfilesView> {
    select_profile(&state, profile_id.as_deref(), expected_revision)
}

fn select_profile(
    state: &AppState,
    profile_id: Option<&str>,
    expected_revision: u64,
) -> ApiResult<ModelProfilesView> {
    match profile_id {
        Some(profile_id) => state.model_profiles.select(profile_id, expected_revision),
        None => state.model_profiles.clear_selection(expected_revision),
    }
}
