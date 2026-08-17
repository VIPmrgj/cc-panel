use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::{
    attachments::import_paths,
    dto::{ApiError, ApiResult, AttachmentImportResult, AttachmentRecord},
    state::{AppState, PendingAttachment},
};

#[tauri::command]
pub async fn pick_and_import_attachments(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<AttachmentImportResult> {
    let selected = app
        .dialog()
        .file()
        .blocking_pick_files()
        .unwrap_or_default();
    let paths = selected
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .collect();
    import_into_state(paths, &state)
}

#[tauri::command]
pub fn import_dropped_attachments(
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> ApiResult<AttachmentImportResult> {
    let paths = paths.into_iter().map(PathBuf::from).collect();
    import_into_state(paths, &state)
}

fn import_into_state(paths: Vec<PathBuf>, state: &AppState) -> ApiResult<AttachmentImportResult> {
    let (existing_count, existing_raw, existing_extracted) = {
        let attachments = state.attachments.lock().expect("attachment store poisoned");
        (
            attachments.len(),
            attachments.values().map(|item| item.record.raw_bytes).sum(),
            attachments
                .values()
                .map(|item| item.record.extracted_bytes)
                .sum(),
        )
    };
    let (result, snapshots) = import_paths(paths, existing_count, existing_raw, existing_extracted);
    let mut attachments = state.attachments.lock().expect("attachment store poisoned");
    let mut pending = state
        .pending_attachments
        .lock()
        .expect("pending attachment store poisoned");
    pending.retain(|_, value| value.expires_at > Instant::now());
    for imported in snapshots {
        let handle = imported.snapshot.record.handle.clone();
        if let Some(reason) = imported.sensitive_reason {
            pending.insert(
                handle.clone(),
                PendingAttachment {
                    token: handle,
                    expires_at: Instant::now() + Duration::from_secs(300),
                    snapshot: imported.snapshot,
                    reason,
                },
            );
        } else {
            attachments.insert(handle, imported.snapshot);
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn confirm_sensitive_import(
    confirmation_token: String,
    state: State<'_, AppState>,
) -> ApiResult<AttachmentRecord> {
    let pending = state
        .pending_attachments
        .lock()
        .expect("pending attachment store poisoned")
        .remove(&confirmation_token)
        .ok_or_else(|| {
            ApiError::new(
                "SENSITIVE_CONFIRMATION_EXPIRED",
                "敏感附件确认已过期，请重新选择文件。",
                false,
            )
        })?;
    if pending.expires_at <= Instant::now() {
        return Err(ApiError::new(
            "SENSITIVE_CONFIRMATION_EXPIRED",
            "敏感附件确认已过期，请重新选择文件。",
            false,
        ));
    }
    let record = pending.snapshot.record.clone();
    state
        .attachments
        .lock()
        .expect("attachment store poisoned")
        .insert(record.handle.clone(), pending.snapshot);
    Ok(record)
}

#[tauri::command]
pub fn remove_attachment(handle: String, state: State<'_, AppState>) -> ApiResult<()> {
    state
        .attachments
        .lock()
        .expect("attachment store poisoned")
        .remove(&handle);
    state
        .pending_attachments
        .lock()
        .expect("pending attachment store poisoned")
        .remove(&handle);
    Ok(())
}

#[tauri::command]
pub fn clear_attachments(state: State<'_, AppState>) -> ApiResult<()> {
    state
        .attachments
        .lock()
        .expect("attachment store poisoned")
        .clear();
    state
        .pending_attachments
        .lock()
        .expect("pending attachment store poisoned")
        .clear();
    Ok(())
}
