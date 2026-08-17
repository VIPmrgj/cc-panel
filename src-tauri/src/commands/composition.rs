use std::{fs, path::Path};

use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

use crate::{
    dto::{ApiError, ApiResult, CompositionRequest, CompositionResult, CopyResult},
    prompt::{compose_prompt, CompositionSkill},
    state::AppState,
};

#[tauri::command]
pub fn compose_preview(
    request: CompositionRequest,
    state: State<'_, AppState>,
) -> ApiResult<CompositionResult> {
    build_composition(&request, &state)
}

#[tauri::command]
pub fn compose_and_copy(
    request: CompositionRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<CopyResult> {
    let composition = build_composition(&request, &state)?;
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

fn build_composition(
    request: &CompositionRequest,
    state: &AppState,
) -> ApiResult<CompositionResult> {
    let inventory_guard = state
        .skill_inventory
        .read()
        .expect("skill inventory poisoned");
    let inventory = inventory_guard.as_ref().ok_or_else(|| {
        ApiError::new(
            "STALE_SKILL_INVENTORY",
            "Skill 清单尚未加载，请先刷新。",
            true,
        )
    })?;
    let mut selected_skills = Vec::new();
    for selected in &request.selected_skills {
        let record = inventory
            .skills
            .iter()
            .find(|skill| skill.instance_id == selected.instance_id)
            .ok_or_else(|| {
                ApiError::new(
                    "STALE_SKILL_INVENTORY",
                    "所选 Skill 已不在当前清单中，请刷新。",
                    true,
                )
            })?;
        if record.manifest_hash != selected.manifest_hash {
            return Err(ApiError::new(
                "STALE_SKILL_INVENTORY",
                "所选 Skill 自清单加载后已改变，请刷新。",
                true,
            ));
        }
        let manifest_path = Path::new(&record.manifest_path);
        let bytes = fs::read(manifest_path).map_err(|_| {
            ApiError::new(
                "STALE_SKILL_INVENTORY",
                "无法重新读取所选 Skill，请刷新清单。",
                true,
            )
        })?;
        if hash(&bytes) != record.manifest_hash {
            return Err(ApiError::new(
                "STALE_SKILL_INVENTORY",
                "所选 Skill 自清单加载后已改变，请刷新。",
                true,
            ));
        }
        let manifest = String::from_utf8(bytes).map_err(|_| {
            ApiError::new(
                "SKILL_MANIFEST_INVALID_UTF8",
                "所选 Skill 清单不是有效 UTF-8。",
                false,
            )
        })?;
        selected_skills.push(CompositionSkill {
            record: record.clone(),
            manifest,
        });
    }
    drop(inventory_guard);

    let attachments_guard = state.attachments.lock().expect("attachment store poisoned");
    let mut ordered_attachments = Vec::new();
    for handle in &request.attachment_handles {
        let snapshot = attachments_guard.get(handle).ok_or_else(|| {
            ApiError::new("STALE_ATTACHMENT_HANDLE", "附件已失效，请重新导入。", true)
        })?;
        ordered_attachments.push(snapshot);
    }
    compose_prompt(
        &request.original_prompt,
        request.enhanced_prompt.as_deref(),
        request.use_enhanced,
        selected_skills,
        &ordered_attachments,
    )
}

fn hash(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}
