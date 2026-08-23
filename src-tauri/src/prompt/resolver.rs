use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use crate::{
    dto::{ApiError, ApiResult, CompositionRequest, CompositionResult, SkillOverrideState},
    state::AppState,
};

use super::{compose_prompt_with_memory, CompositionSkill};

/// Resolves the current Skill inventory and in-memory attachment snapshots, then
/// delegates to the deterministic XML composer. Preview, clipboard copy, and
/// Claude session sends must all use this function so their validation and
/// output stay identical.
pub fn resolve_composition(
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

    let mut selected_skills = Vec::with_capacity(request.selected_skills.len());
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
        if matches!(record.override_state, SkillOverrideState::Off) {
            return Err(ApiError::new(
                "SKILL_DISABLED",
                "所选 Skill 已关闭，请重新开启后再选择。",
                true,
            ));
        }
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
    let mut ordered_attachments = Vec::with_capacity(request.attachment_handles.len());
    for handle in &request.attachment_handles {
        let snapshot = attachments_guard.get(handle).ok_or_else(|| {
            ApiError::new("STALE_ATTACHMENT_HANDLE", "附件已失效，请重新导入。", true)
        })?;
        ordered_attachments.push(snapshot);
    }

    let project_memory = state
        .project_memory
        .load_for_project(state.project_root().as_deref())?;

    compose_prompt_with_memory(
        &request.original_prompt,
        request.enhanced_prompt.as_deref(),
        request.use_enhanced,
        project_memory.as_ref(),
        selected_skills,
        &ordered_attachments,
    )
}

fn hash(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}
