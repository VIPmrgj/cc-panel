use tauri::State;

use crate::{
    dto::{ApiError, ApiResult, SkillInventory},
    skills::SkillRoot,
    state::AppState,
};

#[tauri::command]
pub async fn refresh_skills(state: State<'_, AppState>) -> ApiResult<SkillInventory> {
    build_skill_inventory(&state).await
}

pub async fn build_skill_inventory(state: &AppState) -> ApiResult<SkillInventory> {
    let preferences = state.config.preferences();
    let mut roots = vec![SkillRoot::user(state.paths.user_skills().to_path_buf())];
    if let Some(project) = preferences.selected_project_root.as_ref() {
        roots.push(SkillRoot::project(std::path::Path::new(&project.path)));
    }
    for additional in &preferences.additional_roots {
        roots.push(SkillRoot::additional(
            std::path::Path::new(&additional.path),
            &additional.label,
        ));
    }

    let (claude_cli_available, plugin_warning) = match state.plugin_cli.enabled_roots().await {
        Ok(plugin_roots) => {
            roots.extend(plugin_roots.into_iter().map(SkillRoot::plugin));
            (true, None)
        }
        Err(error) => (false, Some(error.message)),
    };
    let (overrides, settings_revision) = state.settings.all_skill_overrides()?;
    let inventory = state.skill_scanner.scan(
        &roots,
        &overrides,
        settings_revision,
        claude_cli_available,
        plugin_warning,
    );
    *state
        .skill_inventory
        .write()
        .expect("skill inventory poisoned") = Some(inventory.clone());
    Ok(inventory)
}

#[tauri::command]
pub async fn set_skill_override(
    canonical_id: String,
    value: String,
    settings_revision: String,
    state: State<'_, AppState>,
) -> ApiResult<SkillInventory> {
    if !state.known_skill(&canonical_id) {
        return Err(ApiError::new(
            "STALE_SKILL_INVENTORY",
            "该 Skill 不在当前清单中。请刷新后重试。",
            true,
        ));
    }
    state
        .settings
        .set_skill_override(&canonical_id, &value, &settings_revision)?;
    build_skill_inventory(&state).await
}
