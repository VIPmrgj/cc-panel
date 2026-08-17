use std::path::PathBuf;

use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::{
    dto::{ApiError, ApiResult, RootEntry},
    state::AppState,
};

#[tauri::command]
pub async fn choose_project_root(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<Option<RootEntry>> {
    let path = app.dialog().file().blocking_pick_folder();
    let Some(path) = path else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|_| ApiError::new("INVALID_ROOT_PATH", "所选项目目录路径无效。", false))?;
    let canonical = canonical_directory(path)?;
    state.config.set_project_root(Some(&canonical))
}

#[tauri::command]
pub fn clear_project_root(state: State<'_, AppState>) -> ApiResult<()> {
    state.config.set_project_root(None)?;
    Ok(())
}

#[tauri::command]
pub async fn choose_additional_root(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<Option<RootEntry>> {
    let path = app.dialog().file().blocking_pick_folder();
    let Some(path) = path else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|_| ApiError::new("INVALID_ROOT_PATH", "所选附加目录路径无效。", false))?;
    let canonical = canonical_directory(path)?;
    state.config.add_additional_root(&canonical).map(Some)
}

#[tauri::command]
pub fn remove_additional_root(root_id: String, state: State<'_, AppState>) -> ApiResult<()> {
    state.config.remove_additional_root(&root_id)
}

fn canonical_directory(path: PathBuf) -> ApiResult<PathBuf> {
    let canonical = dunce::canonicalize(path)
        .map_err(|_| ApiError::new("ROOT_UNAVAILABLE", "无法访问所选目录。", true))?;
    if !canonical.is_dir() {
        return Err(ApiError::new(
            "ROOT_NOT_DIRECTORY",
            "所选路径不是目录。",
            false,
        ));
    }
    Ok(canonical)
}
