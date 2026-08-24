use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::Value;
use tauri::State;

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
        cc_switch_installed: cc_switch_executable().is_some(),
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
                "Claude Code operation failed.",
                true,
            )
        })?;
        if !output.status.success() {
            return Err(ApiError::new(
                "CLAUDE_INSTALL_FAILED",
                "Claude Code installation failed.",
                true,
            ));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err(ApiError::new(
            "CLAUDE_INSTALL_UNSUPPORTED",
            "Claude Code installation is supported on Windows only.",
            false,
        ))
    }
}

#[tauri::command]
pub fn start_claude_login() -> ApiResult<()> {
    let executable = resolve_claude_executable()
        .ok_or_else(|| ApiError::new("CLAUDE_NOT_INSTALLED", "Claude Code was not found.", true))?;
    let mut command = Command::new(executable);
    command.args(["auth", "login"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE
    }
    command
        .spawn()
        .map_err(|_| ApiError::new("CLAUDE_LOGIN_FAILED", "Claude Code operation failed.", true))?;
    Ok(())
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
    let script = format!("$user=[Environment]::GetEnvironmentVariable('Path','User'); $machine=[Environment]::GetEnvironmentVariable('Path','Machine'); $env:Path=\"$user;$machine;$env:ProgramFiles\\nodejs;$env:APPDATA\\npm;$env:ProgramFiles\\Git\\cmd;$env:LOCALAPPDATA\\Programs\\Git\\cmd\"; & {tool} --version");
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
fn cc_switch_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(value) = env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(value);
        candidates.extend([
            root.join("Programs")
                .join("CC Switch")
                .join("CC Switch.exe"),
            root.join("Programs")
                .join("CC-Switch")
                .join("CC-Switch.exe"),
            root.join("CC Switch").join("CC Switch.exe"),
            root.join("CC-Switch").join("CC-Switch.exe"),
        ]);
    }
    if let Some(value) = env::var_os("ProgramFiles") {
        let root = PathBuf::from(value);
        candidates.extend([
            root.join("CC Switch").join("CC Switch.exe"),
            root.join("CC-Switch").join("CC-Switch.exe"),
        ]);
    }
    candidates.into_iter().find(|path| is_regular_file(path))
}
fn is_regular_file(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    metadata.is_file() && !metadata.file_type().is_symlink()
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
  $env:Path=(($userPath,$machinePath)+$extra|Where-Object {$_}|Select-Object -Unique)-join ';'
}
function Has-Tool([string]$name){$null -ne (Get-Command $name -ErrorAction SilentlyContinue)}
Refresh-CCPath
if(-not(Has-Tool 'node')){if(-not(Has-Tool 'winget')){throw 'winget: winget is not available'}; winget install --id OpenJS.NodeJS.LTS --exact --silent --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){throw 'node: Node.js installation failed'};Refresh-CCPath}
if(-not(Has-Tool 'git')){if(-not(Has-Tool 'winget')){throw 'winget: winget is not available'}; winget install --id Git.Git --exact --silent --accept-source-agreements --accept-package-agreements;if($LASTEXITCODE -ne 0){throw 'git: Git installation failed'};Refresh-CCPath}
if(-not(Has-Tool 'npm')){throw 'npm: npm was not found after Node.js installation'}
npm.cmd config set registry 'https://registry.npmmirror.com/' --global;if($LASTEXITCODE -ne 0){throw 'npm: npm registry configuration failed'}
npm.cmd install --global @anthropic-ai/claude-code --registry 'https://registry.npmmirror.com/';if($LASTEXITCODE -ne 0){throw 'claude-code: Claude Code installation failed'}
Refresh-CCPath
if(-not(Has-Tool 'claude')){throw 'claude-code: Claude executable was not found'}
$ccCandidates=@(($env:LOCALAPPDATA+'\Programs\CC Switch\CC Switch.exe'),($env:LOCALAPPDATA+'\Programs\CC-Switch\CC-Switch.exe'),($env:LOCALAPPDATA+'\CC Switch\CC Switch.exe'),($env:LOCALAPPDATA+'\CC-Switch\CC-Switch.exe'),($env:ProgramFiles+'\CC Switch\CC Switch.exe'),($env:ProgramFiles+'\CC-Switch\CC-Switch.exe'))
if(-not($ccCandidates|Where-Object{Test-Path -LiteralPath $_})){
 $headers=@{'User-Agent'='CC-Panel';'Accept'='application/vnd.github+json'}
 $release=Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/farion1231/cc-switch/releases/latest'
 $asset=$release.assets|Where-Object{$_.name -match '(?i)windows.*\.msi$' -or $_.name -match '(?i)\.msi$'}|Select-Object -First 1
 if($null -eq $asset){throw 'cc-switch: no Windows MSI was found in the latest release'}
 $temp=Join-Path $env:TEMP ('cc-panel-cc-switch-'+[Guid]::NewGuid().ToString()+'.msi')
 try{Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $temp;$installer=Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i',$temp,'/qn','/norestart') -Wait -PassThru;if($installer.ExitCode -notin @(0,3010)){throw 'cc-switch: MSI installation failed'}}finally{Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue}
}"#;
#[cfg(not(windows))]
const DOMESTIC_INSTALL_SCRIPT: &str = "";
#[tauri::command]
pub fn start_cc_switch() -> ApiResult<()> {
    let executable = cc_switch_executable().ok_or_else(|| {
        ApiError::new(
            "CC_SWITCH_NOT_INSTALLED",
            "CC-Switch is not installed.",
            true,
        )
    })?;
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0010);
    }
    command.spawn().map_err(|_| {
        ApiError::new(
            "CC_SWITCH_START_FAILED",
            "CC-Switch could not be started.",
            true,
        )
    })?;
    Ok(())
}
fn configure_hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
}
#[tauri::command]
pub async fn install_domestic_environment() -> ApiResult<()> {
    #[cfg(windows)]
    {
        let output = run_powershell(DOMESTIC_INSTALL_SCRIPT).map_err(|_| {
            ApiError::new(
                "CLAUDE_INSTALL_FAILED",
                "The Windows installation script could not be started.",
                true,
            )
        })?;
        if !output.status.success() {
            return Err(ApiError::new(
                "DOMESTIC_INSTALL_FAILED",
                "The domestic Claude environment installation failed.",
                true,
            ));
        }
        mark_claude_onboarding_complete()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(ApiError::new(
            "CLAUDE_INSTALL_UNSUPPORTED",
            "The domestic installation is supported on Windows only.",
            false,
        ))
    }
}
