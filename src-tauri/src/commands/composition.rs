use tauri::State;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

use crate::{
    dto::{ApiError, ApiResult, CompositionRequest, CompositionResult, CopyResult},
    prompt::resolve_composition,
    state::AppState,
};

#[tauri::command]
pub fn compose_preview(
    request: CompositionRequest,
    state: State<'_, AppState>,
) -> ApiResult<CompositionResult> {
    resolve_composition(&request, &state)
}

#[tauri::command]
pub fn compose_and_copy(
    request: CompositionRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<CopyResult> {
    let composition = resolve_composition(&request, &state)?;
    app.clipboard()
        .write_text(&composition.text)
        .map_err(|_| ApiError::new("CLIPBOARD_WRITE_FAILED", "无法写入系统剪贴板。", true))?;

    let mut notification_sent = false;
    let mut notification_warning = None;
    if state.config.preferences().native_notifications_enabled {
        match app
            .notification()
            .builder()
            .title("CC Panel")
            .body("最终 Prompt 已复制到剪贴板")
            .show()
        {
            Ok(()) => notification_sent = true,
            Err(_) => notification_warning = Some("Prompt 已复制，但系统通知发送失败。".into()),
        }
    }
    Ok(CopyResult {
        composition,
        copied: true,
        notification_sent,
        notification_warning,
    })
}

#[tauri::command]
pub fn set_native_notifications_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> ApiResult<bool> {
    state.config.set_notifications(enabled)?;
    Ok(enabled)
}
