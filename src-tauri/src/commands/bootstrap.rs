use std::process::{Command, Stdio};

use tauri::State;

use crate::{
    dto::{ApiResult, BootstrapResponse},
    platform::resolve_claude_executable,
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
    let executable = resolve_claude_executable()?;
    let mut command = Command::new(executable);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW：隐藏 Claude CLI 控制台窗口
    }
    let output = command.output().ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}
