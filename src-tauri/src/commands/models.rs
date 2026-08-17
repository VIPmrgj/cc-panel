use tauri::State;

use crate::{
    dto::{ApiResult, ModelStatus},
    state::AppState,
};

#[tauri::command]
pub fn get_model_status(state: State<'_, AppState>) -> ApiResult<ModelStatus> {
    let project_root = state.project_root();
    state.settings.model_status(project_root.as_deref())
}

#[tauri::command]
pub fn set_user_model(
    model: String,
    settings_revision: String,
    state: State<'_, AppState>,
) -> ApiResult<ModelStatus> {
    let project_root = state.project_root();
    state
        .settings
        .set_user_model(&model, &settings_revision, project_root.as_deref())
}

#[tauri::command]
pub fn clear_user_model(
    settings_revision: String,
    state: State<'_, AppState>,
) -> ApiResult<ModelStatus> {
    let project_root = state.project_root();
    state
        .settings
        .clear_user_model(&settings_revision, project_root.as_deref())
}
