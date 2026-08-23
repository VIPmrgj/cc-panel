use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::{
    attachments::import_paths,
    dto::{ApiError, ApiResult, AttachmentImportResult, AttachmentPreview, AttachmentRecord},
    state::{AppState, DroppedAttachmentGrant, PendingAttachment},
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

const MAX_PREVIEW_CHARS: usize = 50_000;

#[tauri::command]
pub fn preview_attachment(
    handle: String,
    state: State<'_, AppState>,
) -> ApiResult<AttachmentPreview> {
    let attachments = state.attachments.lock().expect("attachment store poisoned");
    let snapshot = attachments
        .get(&handle)
        .ok_or_else(|| ApiError::new("ATTACHMENT_NOT_FOUND", "附件已失效，请重新导入。", true))?;

    let truncated = snapshot.content.chars().count() > MAX_PREVIEW_CHARS;
    let mut content: String = snapshot.content.chars().take(MAX_PREVIEW_CHARS).collect();
    if truncated {
        content.push_str("\n\n[预览仅显示前 50,000 个字符]");
    }

    let data_url = snapshot
        .preview_bytes
        .as_ref()
        .map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes),));

    Ok(AttachmentPreview {
        attachment: snapshot.record.clone(),
        content,
        truncated,
        data_url,
    })
}
/// Import only paths granted by the native OS drag/drop event listener. The
/// grant is minted in Rust and expires after one short-lived request window;
/// arbitrary invoke callers cannot mint one by supplying a path string.
#[tauri::command]
pub fn import_dropped_attachments(
    grant: String,
    state: State<'_, AppState>,
) -> ApiResult<AttachmentImportResult> {
    if grant.trim().is_empty() {
        return Err(ApiError::new(
            "ATTACHMENT_DROP_INVALID",
            "拖放附件请求无效。",
            false,
        ));
    }
    let dropped = consume_drop_grant(&grant, &state.dropped_attachment_grants)?;
    import_into_state(dropped.paths, &state)
}

pub fn grant_dropped_attachments(state: &AppState, paths: Vec<PathBuf>) -> Option<String> {
    if paths.is_empty() || paths.len() > 10 {
        return None;
    }
    let grant = uuid::Uuid::new_v4().to_string();
    let mut grants = state
        .dropped_attachment_grants
        .lock()
        .expect("attachment grant store poisoned");
    let now = Instant::now();
    grants.retain(|_, dropped| dropped.expires_at > now);
    grants.insert(
        grant.clone(),
        DroppedAttachmentGrant {
            expires_at: now + Duration::from_secs(10),
            paths,
        },
    );
    Some(grant)
}

fn consume_drop_grant(
    grant: &str,
    grants: &Mutex<HashMap<String, DroppedAttachmentGrant>>,
) -> ApiResult<DroppedAttachmentGrant> {
    let mut grants = grants.lock().expect("attachment grant store poisoned");
    let now = Instant::now();
    grants.retain(|_, dropped| dropped.expires_at > now);
    grants.remove(grant).ok_or_else(|| {
        ApiError::new(
            "ATTACHMENT_DROP_EXPIRED",
            "附件拖放授权已过期，请重新拖放。",
            false,
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_grant_returns_only_the_paths_bound_at_creation() {
        let grants = Mutex::new(HashMap::from([(
            "grant-1".to_string(),
            DroppedAttachmentGrant {
                expires_at: Instant::now() + Duration::from_secs(10),
                paths: vec![PathBuf::from("native.txt")],
            },
        )]));

        let dropped = consume_drop_grant("grant-1", &grants).expect("grant should be valid");

        assert_eq!(dropped.paths, vec![PathBuf::from("native.txt")]);
    }

    #[test]
    fn drop_grant_is_single_use() {
        let grants = Mutex::new(HashMap::from([(
            "grant-1".to_string(),
            DroppedAttachmentGrant {
                expires_at: Instant::now() + Duration::from_secs(10),
                paths: vec![PathBuf::from("native.txt")],
            },
        )]));

        consume_drop_grant("grant-1", &grants).expect("first use should succeed");
        let error = consume_drop_grant("grant-1", &grants).expect_err("replay must fail");

        assert_eq!(error.code, "ATTACHMENT_DROP_EXPIRED");
    }

    #[test]
    fn expired_drop_grant_is_rejected() {
        let grants = Mutex::new(HashMap::from([(
            "grant-1".to_string(),
            DroppedAttachmentGrant {
                expires_at: Instant::now() - Duration::from_millis(1),
                paths: vec![PathBuf::from("native.txt")],
            },
        )]));

        let error = consume_drop_grant("grant-1", &grants).expect_err("expired grant must fail");

        assert_eq!(error.code, "ATTACHMENT_DROP_EXPIRED");
        assert!(grants.lock().expect("grant store poisoned").is_empty());
    }
}
