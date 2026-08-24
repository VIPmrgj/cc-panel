#[cfg(windows)]
use std::env;
use std::{
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
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
        powershell_available: powershell_available(),
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
    for bytes in [&output.stderr, &output.stdout] {
        let text = String::from_utf8_lossy(bytes);
        for line in text.lines().rev() {
            let line = line.trim();
            if line.is_empty() || is_powershell_progress(line) {
                continue;
            }
            return Some(line.chars().take(320).collect());
        }
    }
    None
}

fn is_powershell_progress(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("clixml")
        || lower.contains("<objs version=")
        || lower.contains("<obj s=\"progress\"")
        || lower.contains("system.management.automation.pscustomobject")
        || lower.contains("<pr n=\"record\">")
        || lower.contains("fullyqualifiederrorid")
}

fn powershell_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(windows)]
    {
        for variable in ["SystemRoot", "WINDIR"] {
            if let Some(root) = env::var_os(variable) {
                let root = PathBuf::from(root);
                candidates.push(
                    root.join("System32")
                        .join("WindowsPowerShell")
                        .join("v1.0")
                        .join("powershell.exe"),
                );
                candidates.push(
                    root.join("Sysnative")
                        .join("WindowsPowerShell")
                        .join("v1.0")
                        .join("powershell.exe"),
                );
            }
        }

        if let Some(program_files) = env::var_os("ProgramFiles") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("PowerShell")
                    .join("7")
                    .join("pwsh.exe"),
            );
        }
    }

    candidates.extend([PathBuf::from("powershell.exe"), PathBuf::from("pwsh.exe")]);
    candidates.dedup();
    candidates
}

fn encode_powershell_script(script: &str) -> String {
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    BASE64.encode(bytes)
}

fn powershell_command(executable: &Path, encoded: &str) -> Command {
    let mut command = Command::new(executable);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            encoded,
        ])
        .stdin(Stdio::null());
    configure_hidden(&mut command);
    command
}

fn powershell_start_error(errors: &[String]) -> io::Error {
    let detail = if errors.is_empty() {
        "没有找到可启动的 PowerShell 程序".to_owned()
    } else {
        errors.join("；")
    };
    io::Error::other(format!("PowerShell 启动失败：{detail}"))
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
    let encoded = encode_powershell_script(script);
    let mut child = None;
    let mut errors = Vec::new();
    for executable in powershell_candidates() {
        let mut command = powershell_command(&executable, &encoded);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        match command.spawn() {
            Ok(process) => {
                child = Some(process);
                break;
            }
            Err(error) => errors.push(format!("{}：{}", executable.display(), error)),
        }
    }
    let mut child = child.ok_or_else(|| powershell_start_error(&errors))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("PowerShell 标准输出管道创建失败"))?;
    let stderr_handle = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut stderr = Vec::new();
            pipe.read_to_end(&mut stderr).map(|_| stderr)
        })
    });
    let mut current_step = None;
    let mut stdout_lines = Vec::new();
    let mut reader = BufReader::new(stdout);
    let mut raw_line = Vec::new();
    loop {
        raw_line.clear();
        if reader.read_until(b'\n', &mut raw_line)? == 0 {
            break;
        }
        let line = decode_powershell_line(&raw_line);
        let Some(raw_step) = line.strip_prefix("CCP_STEP=") else {
            if line.starts_with("CCP_ERROR=") {
                stdout_lines.clear();
                stdout_lines.push(line);
            } else if !line.trim().is_empty() && stdout_lines.len() < 64 {
                stdout_lines.push(line);
            }
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
            stdout: stdout_lines.join("\n").into_bytes(),
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
    let encoded = encode_powershell_script(script);
    let mut errors = Vec::new();
    for executable in powershell_candidates() {
        match powershell_command(&executable, &encoded).output() {
            Ok(output) => return Ok(output),
            Err(error) => errors.push(format!("{}：{}", executable.display(), error)),
        }
    }
    Err(powershell_start_error(&errors))
}

fn powershell_available() -> bool {
    run_powershell("$null").is_ok_and(|output| output.status.success())
}
#[cfg(windows)]
const DOMESTIC_INSTALL_SCRIPT: &str = r#"$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';$OutputEncoding=[Text.UTF8Encoding]::new($false);[Console]::OutputEncoding=$OutputEncoding
function Refresh-CCPath {
  $userPath=[Environment]::GetEnvironmentVariable('Path','User'); $machinePath=[Environment]::GetEnvironmentVariable('Path','Machine')
  $extra=@($env:ProgramFiles+'\nodejs',$env:APPDATA+'\npm',$env:ProgramFiles+'\Git\cmd',$env:LOCALAPPDATA+'\Programs\Git\cmd')
  $env:Path=(($extra,$userPath,$machinePath)|Where-Object {$_}|Select-Object -Unique)-join ';'
}
function Has-Tool([string]$name){$null -ne (Get-Command $name -ErrorAction SilentlyContinue)}
function Require-Winget { if(-not(Has-Tool 'winget')){Write-Output 'CCP_ERROR=Windows 包管理器 winget 不可用，请先安装 App Installer';exit 31} }
Write-Output 'CCP_STEP=node'
Refresh-CCPath
if(-not(Has-Tool 'node')){Require-Winget; $null = winget.exe install --id OpenJS.NodeJS.LTS --exact --source winget --silent --disable-interactivity --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){Write-Output ("CCP_ERROR=Node.js 安装失败，winget 返回代码："+$LASTEXITCODE);exit 32};Refresh-CCPath}
Write-Output 'CCP_STEP=git'
if(-not(Has-Tool 'git')){Require-Winget; $null = winget.exe install --id Git.Git --exact --source winget --silent --disable-interactivity --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){Write-Output ("CCP_ERROR=Git 安装失败，winget 返回代码："+$LASTEXITCODE);exit 33};Refresh-CCPath}
Write-Output 'CCP_STEP=npm'
if(-not(Has-Tool 'npm')){Write-Output 'CCP_ERROR=Node.js 安装后没有找到 npm';exit 41}
$null = npm.cmd config set registry 'https://registry.npmmirror.com/' --global;if($LASTEXITCODE -ne 0){Write-Output ("CCP_ERROR=npm 镜像配置失败，返回代码："+$LASTEXITCODE);exit 42}
$registry=(npm.cmd config get registry).Trim().TrimEnd('/')
if($registry -ine 'https://registry.npmmirror.com'){Write-Output 'CCP_ERROR=npm 国内镜像验证失败';exit 43}
Write-Output 'CCP_STEP=claude'
$null = npm.cmd install --global '@anthropic-ai/claude-code@latest' --registry 'https://registry.npmmirror.com/';if($LASTEXITCODE -ne 0){Write-Output ("CCP_ERROR=Claude Code 安装失败，npm 返回代码："+$LASTEXITCODE);exit 44}
Refresh-CCPath
if(-not(Has-Tool 'claude')){Write-Output 'CCP_ERROR=安装后没有找到 Claude Code';exit 45}
$null = claude --version;if($LASTEXITCODE -ne 0){Write-Output 'CCP_ERROR=Claude Code 安装后无法运行';exit 46}"#;
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
            .map_err(|error| {
                let message = format!("国内环境安装脚本无法启动：{error}");
                emit_domestic_install_progress(&app, 1, "node", "failed", Some(&message));
                ApiError::new("DOMESTIC_INSTALL_FAILED", message, true)
            })?;
        if !output.status.success() {
            let step = current_step.unwrap_or(1);
            let (phase, failed_message) = domestic_step_info(step);
            let exit_code = output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_owned());
            let message = command_output_detail(&output)
                .map(|detail| format!("{failed_message}（{detail}）"))
                .unwrap_or_else(|| {
                    format!("{failed_message}，Windows 安装程序返回代码：{exit_code}。")
                });
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

fn decode_powershell_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(&['\r', '\n'][..])
        .to_owned()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_progress_xml_is_filtered() {
        assert!(is_powershell_progress(
            r#"<Objs Version="1.1.0.1"><Obj S="progress"><T>Completed</T></Obj></Objs>"#
        ));
        assert!(is_powershell_progress("#< CLIXML"));
        assert!(!is_powershell_progress("Node.js installation failed"));
    }

    #[test]
    fn powershell_candidates_include_fallback_executables() {
        let candidates = powershell_candidates();
        assert!(candidates
            .iter()
            .any(|path| path.ends_with("powershell.exe")));
        assert!(candidates.iter().any(|path| path.ends_with("pwsh.exe")));
    }

    #[test]
    fn powershell_start_error_preserves_attempt_details() {
        let error = powershell_start_error(&["powershell.exe: access denied".to_owned()]);
        assert!(error.to_string().contains("access denied"));
    }

    #[test]
    fn powershell_line_decoder_accepts_non_utf8() {
        let line = decode_powershell_line(&[b'C', b'C', b'P', b'_', 0xff, b'\n']);
        assert!(line.starts_with("CCP_"));
        assert!(!line.contains('\n'));
    }
    #[cfg(windows)]
    #[test]
    fn domestic_script_does_not_require_cc_switch() {
        assert!(!DOMESTIC_INSTALL_SCRIPT.contains("CC-Switch"));
        assert!(DOMESTIC_INSTALL_SCRIPT.contains("--source winget"));
        assert!(DOMESTIC_INSTALL_SCRIPT.contains("--disable-interactivity"));
        assert!(DOMESTIC_INSTALL_SCRIPT.contains("$OutputEncoding"));
        assert!(DOMESTIC_INSTALL_SCRIPT.contains("$null = winget.exe"));
        assert!(DOMESTIC_INSTALL_SCRIPT.contains("$null = npm.cmd install"));
    }
}
