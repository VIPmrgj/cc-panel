use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Command, Output, Stdio},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::Value;
use tauri::{Emitter, State};

use crate::{
    dto::{ApiError, ApiResult, BootstrapResponse},
    platform::{replace_file_atomically, resolve_claude_executable},
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
    let (claude_code_version, claude_code_authenticated, claude_code_configured) = detect_claude();
    Ok(BootstrapResponse {
        preferences,
        model,
        skills,
        ollama,
        attachments: state.attachment_records(),
        claude_code_version,
        claude_code_authenticated,
        claude_code_configured,
        node_version: detect_tool_version("node"),
        npm_version: detect_tool_version("npm"),
        npm_mirror_configured: npm_mirror_configured(),
        git_available: git_available(),
    })
}

const DOMESTIC_INSTALL_PROGRESS_EVENT: &str = "cc-panel://domestic-install-progress";
const DOMESTIC_INSTALL_TOTAL_STEPS: u8 = 5;

fn emit_domestic_install_progress(
    app: &tauri::AppHandle,
    step: u8,
    phase: &str,
    status: &str,
    message: Option<&str>,
) {
    let _ = app.emit(
        DOMESTIC_INSTALL_PROGRESS_EVENT,
        serde_json::json!({
            "step": step,
            "totalSteps": DOMESTIC_INSTALL_TOTAL_STEPS,
            "phase": phase,
            "status": status,
            "message": message,
        }),
    );
}

fn command_output_detail(output: &Output) -> Option<String> {
    let bytes = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let line = String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    if line.is_empty() {
        None
    } else {
        Some(line.chars().take(320).collect())
    }
}

fn domestic_step_info(step: u8) -> (&'static str, &'static str) {
    match step {
        1 => ("node", "正在检查或安装 Node.js"),
        2 => ("git", "正在检查或安装 Git"),
        3 => ("npm", "正在配置 npm 国内镜像"),
        4 => ("claude", "正在安装或更新 Claude Code"),
        _ => ("node", "正在准备 Windows 环境"),
    }
}

fn run_powershell_streaming(
    script: &str,
    app: &tauri::AppHandle,
) -> Result<(Output, Option<u8>), std::io::Error> {
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let encoded = BASE64.encode(bytes);
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_hidden(&mut command);
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("missing PowerShell stdout"))?;
    let stderr_handle = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut stderr = Vec::new();
            pipe.read_to_end(&mut stderr).map(|_| stderr)
        })
    });
    let mut current_step = None;
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        let Some(raw_step) = line.strip_prefix("CCP_STEP=") else {
            continue;
        };
        let Ok(step) = raw_step.trim().parse::<u8>() else {
            continue;
        };
        if !(1..=4).contains(&step) || current_step == Some(step) {
            continue;
        }
        if let Some(previous) = current_step {
            let (phase, message) = domestic_step_info(previous);
            emit_domestic_install_progress(app, previous, phase, "completed", Some(message));
        }
        let (phase, message) = domestic_step_info(step);
        emit_domestic_install_progress(app, step, phase, "running", Some(message));
        current_step = Some(step);
    }

    let status = child.wait()?;
    let stderr = match stderr_handle {
        Some(handle) => match handle.join() {
            Ok(result) => result?,
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    };
    Ok((
        Output {
            status,
            stdout: Vec::new(),
            stderr,
        },
        current_step,
    ))
}
fn detect_claude() -> (Option<String>, bool, bool) {
    let Some(executable) = resolve_claude_executable() else {
        return (None, false, false);
    };
    let version = claude_version(&executable);
    let authenticated = version.is_some() && claude_auth_status(&executable);
    (
        version,
        authenticated,
        authenticated || third_party_claude_configured(),
    )
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
    if detect_tool_version("git").is_some() {
        return true;
    }
    let mut command = Command::new("git");
    command.arg("--version").stdin(Stdio::null());
    configure_hidden(&mut command);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
fn third_party_claude_configured() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let Ok(bytes) = fs::read(home.join(".claude").join("settings.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    let Some(env) = value.get("env").and_then(Value::as_object) else {
        return false;
    };
    env_string(env, "ANTHROPIC_BASE_URL").is_some()
        && (env_string(env, "ANTHROPIC_AUTH_TOKEN").is_some()
            || env_string(env, "ANTHROPIC_API_KEY").is_some())
}
fn env_string(env: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    env.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
fn detect_tool_version(tool: &str) -> Option<String> {
    let script = format!("$user=[Environment]::GetEnvironmentVariable('Path','User'); $machine=[Environment]::GetEnvironmentVariable('Path','Machine'); $extra=@($env:ProgramFiles+'\\nodejs',$env:APPDATA+'\\npm',$env:ProgramFiles+'\\Git\\cmd',$env:LOCALAPPDATA+'\\Programs\\Git\\cmd'); $env:Path=(($extra,$user,$machine)|Where-Object {{$_}}|Select-Object -Unique)-join ';'; & {tool} --version");
    let output = run_powershell(&script).ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}
fn npm_mirror_configured() -> bool {
    let Ok(output) = run_powershell("$env:Path=\"$env:Path;$env:ProgramFiles\\nodejs;$env:APPDATA\\npm\"; npm.cmd config get registry") else { return false };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_end_matches('/')
            .eq_ignore_ascii_case("https://registry.npmmirror.com")
}
fn mark_claude_onboarding_complete() -> ApiResult<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        ApiError::new(
            "HOME_DIRECTORY_UNAVAILABLE",
            "Unable to determine the user home directory.",
            false,
        )
    })?;
    let target = home.join(".claude.json");
    let mut value = if target.exists() {
        let metadata =
            fs::symlink_metadata(&target).map_err(|_| ApiError::io("inspect-claude-profile"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ApiError::new(
                "UNSAFE_CLAUDE_PROFILE",
                "The Claude Code profile must be a regular local file.",
                false,
            ));
        }
        let bytes = fs::read(&target).map_err(|_| ApiError::io("read-claude-profile"))?;
        if bytes.len() > 1024 * 1024 {
            return Err(ApiError::new(
                "CLAUDE_PROFILE_TOO_LARGE",
                "The Claude Code profile is too large to update safely.",
                false,
            ));
        }
        serde_json::from_slice::<Value>(&bytes).map_err(|_| {
            ApiError::new(
                "INVALID_CLAUDE_PROFILE",
                "The Claude Code profile could not be parsed as JSON.",
                false,
            )
        })?
    } else {
        Value::Object(serde_json::Map::new())
    };
    let object = value.as_object_mut().ok_or_else(|| {
        ApiError::new(
            "INVALID_CLAUDE_PROFILE",
            "The Claude Code profile must contain a JSON object.",
            false,
        )
    })?;
    object.insert("hasCompletedOnboarding".into(), Value::Bool(true));
    let bytes =
        serde_json::to_vec_pretty(&value).map_err(|_| ApiError::io("serialize-claude-profile"))?;
    replace_file_atomically(&target, &bytes)
}
fn run_powershell(script: &str) -> Result<Output, std::io::Error> {
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes())
    }
    let encoded = BASE64.encode(bytes);
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .stdin(Stdio::null());
    configure_hidden(&mut command);
    command.output()
}
#[cfg(windows)]
const DOMESTIC_INSTALL_SCRIPT: &str = r#"$ErrorActionPreference='Stop'
function Refresh-CCPath {
  $userPath=[Environment]::GetEnvironmentVariable('Path','User'); $machinePath=[Environment]::GetEnvironmentVariable('Path','Machine')
  $extra=@($env:ProgramFiles+'\nodejs',$env:APPDATA+'\npm',$env:ProgramFiles+'\Git\cmd',$env:LOCALAPPDATA+'\Programs\Git\cmd')
  $env:Path=(($extra,$userPath,$machinePath)|Where-Object {$_}|Select-Object -Unique)-join ';'
}
function Has-Tool([string]$name){$null -ne (Get-Command $name -ErrorAction SilentlyContinue)}
Write-Output 'CCP_STEP=node'
Refresh-CCPath
if(-not(Has-Tool 'node')){if(-not(Has-Tool 'winget')){throw 'winget is not available'}; winget.exe install --id OpenJS.NodeJS.LTS --exact --silent --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){throw 'Node.js installation failed'};Refresh-CCPath}
Write-Output 'CCP_STEP=git'
if(-not(Has-Tool 'git')){if(-not(Has-Tool 'winget')){throw 'winget is not available'}; winget.exe install --id Git.Git --exact --silent --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){throw 'Git installation failed'};Refresh-CCPath}
Write-Output 'CCP_STEP=npm'
if(-not(Has-Tool 'npm')){throw 'npm was not found after Node.js installation'}
npm.cmd config set registry 'https://registry.npmmirror.com/' --global;if($LASTEXITCODE -ne 0){throw 'npm mirror configuration failed'}
$registry=(npm.cmd config get registry).Trim().TrimEnd('/')
if($registry -ine 'https://registry.npmmirror.com'){throw 'npm mirror verification failed'}
Write-Output 'CCP_STEP=claude'
npm.cmd install --global '@anthropic-ai/claude-code@latest' --registry 'https://registry.npmmirror.com/';if($LASTEXITCODE -ne 0){throw 'Claude Code installation failed'}
Refresh-CCPath
if(-not(Has-Tool 'claude')){throw 'Claude executable was not found after installation'}
claude --version"#;
#[cfg(not(windows))]
const DOMESTIC_INSTALL_SCRIPT: &str = "";

fn configure_hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}
#[tauri::command]
pub async fn install_domestic_environment(app: tauri::AppHandle) -> ApiResult<()> {
    #[cfg(windows)]
    {
        let (output, current_step) = run_powershell_streaming(DOMESTIC_INSTALL_SCRIPT, &app)
            .map_err(|_| {
                emit_domestic_install_progress(
                    &app,
                    1,
                    "node",
                    "failed",
                    Some("The Windows installation script could not be started"),
                );
                ApiError::new(
                    "DOMESTIC_INSTALL_FAILED",
                    "The Windows installation script could not be started.",
                    true,
                )
            })?;
        if !output.status.success() {
            let step = current_step.unwrap_or(1);
            let (phase, failed_message) = domestic_step_info(step);
            let message = command_output_detail(&output)
                .map(|detail| format!("{failed_message} ({detail})"))
                .unwrap_or_else(|| format!("{failed_message}."));
            emit_domestic_install_progress(&app, step, phase, "failed", Some(&message));
            return Err(ApiError::new("DOMESTIC_INSTALL_FAILED", message, true));
        }
        if let Some(step) = current_step {
            let (phase, message) = domestic_step_info(step);
            emit_domestic_install_progress(&app, step, phase, "completed", Some(message));
        }
        emit_domestic_install_progress(
            &app,
            5,
            "onboarding",
            "running",
            Some("Updating Claude Code first-run settings"),
        );
        if let Err(error) = mark_claude_onboarding_complete() {
            emit_domestic_install_progress(
                &app,
                5,
                "onboarding",
                "failed",
                Some("Claude Code first-run settings could not be updated"),
            );
            return Err(error);
        }
        emit_domestic_install_progress(
            &app,
            5,
            "onboarding",
            "completed",
            Some("Claude Code first-run settings are ready"),
        );
        emit_domestic_install_progress(
            &app,
            5,
            "complete",
            "completed",
            Some("Domestic environment preparation completed"),
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(ApiError::new(
            "CLAUDE_INSTALL_UNSUPPORTED",
            "The domestic installation is supported on Windows only.",
            false,
        ))
    }
}
