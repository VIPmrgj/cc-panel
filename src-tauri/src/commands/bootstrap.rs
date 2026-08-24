use std::{
    path::Path,
    process::{Command, Stdio},
};

use tauri::State;

use crate::{
    dto::{ApiError, ApiResult, BootstrapResponse},
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
    let (claude_code_version, claude_code_authenticated) = detect_claude();
    Ok(BootstrapResponse {
        preferences,
        model,
        skills,
        ollama,
        attachments: state.attachment_records(),
        claude_code_version,
        claude_code_authenticated,
        git_available: git_available(),
    })
}

#[tauri::command]
pub async fn install_claude_code() -> ApiResult<()> {
    #[cfg(windows)]
    {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$ErrorActionPreference='Stop'; irm https://claude.ai/install.ps1 | iex",
        ]);
        command.stdin(Stdio::null());
        configure_hidden(&mut command);
        let output = command.output().map_err(|_| {
            ApiError::new(
                "CLAUDE_INSTALL_FAILED",
                "无法启动 Claude Code 安装程序。",
                true,
            )
        })?;
        if !output.status.success() {
            return Err(ApiError::new(
                "CLAUDE_INSTALL_FAILED",
                "Claude Code 安装失败，请检查网络后重试。",
                true,
            ));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err(ApiError::new(
            "CLAUDE_INSTALL_UNSUPPORTED",
            "当前安装向导只支持 Windows。",
            false,
        ))
    }
}

#[tauri::command]
pub fn start_claude_login() -> ApiResult<()> {
    let executable = resolve_claude_executable().ok_or_else(|| {
        ApiError::new(
            "CLAUDE_NOT_INSTALLED",
            "还没有检测到 Claude Code，请先完成安装。",
            true,
        )
    })?;
    let mut command = Command::new(executable);
    command.args(["auth", "login"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE：让用户看到登录提示
    }
    command.spawn().map_err(|_| {
        ApiError::new(
            "CLAUDE_LOGIN_FAILED",
            "无法打开 Claude Code 登录窗口。",
            true,
        )
    })?;
    Ok(())
}

fn detect_claude() -> (Option<String>, bool) {
    let Some(executable) = resolve_claude_executable() else {
        return (None, false);
    };
    let version = claude_version(&executable);
    let authenticated = version.is_some() && claude_auth_status(&executable);
    (version, authenticated)
}

fn claude_version(executable: &Path) -> Option<String> {
    let mut command = Command::new(executable);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    configure_hidden(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}

fn claude_auth_status(executable: &Path) -> bool {
    let mut command = Command::new(executable);
    command.args(["auth", "status"]).stdin(Stdio::null());
    configure_hidden(&mut command);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_available() -> bool {
    let mut command = Command::new("git");
    command.arg("--version").stdin(Stdio::null());
    configure_hidden(&mut command);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn configure_hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}
