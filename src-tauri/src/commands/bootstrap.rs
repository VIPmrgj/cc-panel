use std::process::{Command, Stdio};

use tauri::State;

use crate::{
    dto::{ApiResult, BootstrapResponse},
    state::AppState,
};

use super::{build_ollama_status, build_skill_inventory};

#[tauri::command]
pub async fn get_bootstrap(state: State<'_, AppState>) -> ApiResult<BootstrapResponse> {
    let preferences = state.config.preferences();
    let project_root = state.project_root();
    let model = state.settings.model_status(project_root.as_deref())?;
    let skills = build_skill_inventory(&state).await?;
    let ollama = build_ollama_status(&state).await;
    Ok(BootstrapResponse {
        preferences,
        model,
        skills,
        ollama,
        attachments: state.attachment_records(),
        claude_code_version: claude_version(),
    })
}

fn claude_version() -> Option<String> {
    let output = Command::new("claude")
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}
