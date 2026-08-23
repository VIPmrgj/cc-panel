use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use reqwest::redirect::Policy;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    dto::{
        ApiError, ApiResult, DiagnosticResult, DownloadedUpdate, EnvironmentCheck,
        EnvironmentReport, ProjectMemory, ProjectMemoryInput, UpdateInfo,
    },
    platform::{replace_file_atomically, resolve_claude_executable},
    state::AppState,
};

const MEMORY_LIMIT: usize = 64 * 1024;
const DIAGNOSTIC_LIMIT: usize = 2 * 1024 * 1024;
const UPDATE_DOWNLOAD_LIMIT: usize = 100 * 1024 * 1024;
const UPDATE_API: &str = "https://api.github.com/repos/VIPmrgj/cc-panel/releases/latest";

#[derive(Clone)]
pub struct ProjectMemoryStore {
    directory: PathBuf,
}

impl ProjectMemoryStore {
    pub fn new(directory: PathBuf) -> ApiResult<Self> {
        if directory.exists() {
            let metadata = fs::symlink_metadata(&directory)
                .map_err(|_| ApiError::io("inspect-project-memory"))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(ApiError::new(
                    "UNSAFE_PROJECT_MEMORY_PATH",
                    "项目记忆目录不安全。",
                    false,
                ));
            }
        } else {
            fs::create_dir_all(&directory).map_err(|_| ApiError::io("create-project-memory"))?;
        }
        Ok(Self { directory })
    }

    pub fn load_for_project(&self, project: Option<&Path>) -> ApiResult<Option<ProjectMemory>> {
        let Some(project) = project else {
            return Ok(None);
        };
        let path = self.path_for(project);
        if !path.exists() {
            return Ok(Some(default_memory(project)));
        }
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| ApiError::io("inspect-project-memory"))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MEMORY_LIMIT as u64
        {
            return Err(ApiError::new(
                "INVALID_PROJECT_MEMORY",
                "项目记忆文件无效或过大。",
                false,
            ));
        }
        let bytes = fs::read(path).map_err(|_| ApiError::io("read-project-memory"))?;
        let memory: ProjectMemory = serde_json::from_slice(&bytes).map_err(|_| {
            ApiError::new("INVALID_PROJECT_MEMORY", "项目记忆文件格式无效。", false)
        })?;
        validate_memory(&memory)?;
        Ok(Some(memory))
    }

    pub fn save_for_project(
        &self,
        project: &Path,
        input: ProjectMemoryInput,
    ) -> ApiResult<ProjectMemory> {
        let memory = ProjectMemory {
            project_path: project.to_string_lossy().into_owned(),
            enabled: input.enabled,
            purpose: validate_text(input.purpose, "purpose")?,
            tech_stack: validate_text(input.tech_stack, "techStack")?,
            rules: validate_text(input.rules, "rules")?,
            avoid: validate_text(input.avoid, "avoid")?,
            test_command: validate_text(input.test_command, "testCommand")?,
            preferred_language: validate_text(input.preferred_language, "preferredLanguage")?,
            updated_at_ms: now_ms(),
        };
        validate_memory(&memory)?;
        let bytes = serde_json::to_vec_pretty(&memory).map_err(|_| {
            ApiError::new(
                "PROJECT_MEMORY_SERIALIZATION_FAILED",
                "无法保存项目记忆。",
                false,
            )
        })?;
        if bytes.len() > MEMORY_LIMIT {
            return Err(ApiError::new(
                "PROJECT_MEMORY_TOO_LARGE",
                "项目记忆超过安全大小。",
                false,
            ));
        }
        replace_file_atomically(&self.path_for(project), &bytes)?;
        Ok(memory)
    }

    fn path_for(&self, project: &Path) -> PathBuf {
        let mut digest = Sha256::new();
        digest.update(project.to_string_lossy().as_bytes());
        self.directory
            .join(format!("{}.json", hex::encode(digest.finalize())))
    }
}

fn default_memory(project: &Path) -> ProjectMemory {
    ProjectMemory {
        project_path: project.to_string_lossy().into_owned(),
        enabled: false,
        purpose: String::new(),
        tech_stack: String::new(),
        rules: String::new(),
        avoid: String::new(),
        test_command: String::new(),
        preferred_language: String::new(),
        updated_at_ms: 0,
    }
}

fn validate_memory(memory: &ProjectMemory) -> ApiResult<()> {
    if memory.project_path.is_empty()
        || memory.project_path.len() > 4096
        || memory.project_path.chars().any(char::is_control)
    {
        return Err(ApiError::new(
            "INVALID_PROJECT_MEMORY",
            "项目目录无效。",
            false,
        ));
    }
    for (field, value) in [
        ("purpose", &memory.purpose),
        ("techStack", &memory.tech_stack),
        ("rules", &memory.rules),
        ("avoid", &memory.avoid),
        ("testCommand", &memory.test_command),
        ("preferredLanguage", &memory.preferred_language),
    ] {
        if value.len() > MEMORY_LIMIT || value.chars().any(is_disallowed_control) {
            return Err(ApiError::new(
                "INVALID_PROJECT_MEMORY",
                format!("项目记忆字段 {field} 无效。"),
                false,
            ));
        }
    }
    Ok(())
}

fn is_disallowed_control(character: char) -> bool {
    character.is_control() && !matches!(character, '\n' | '\r' | '\t')
}
fn validate_text(value: String, field: &'static str) -> ApiResult<String> {
    if value.len() > MEMORY_LIMIT || value.chars().any(is_disallowed_control) {
        return Err(ApiError::new(
            "INVALID_PROJECT_MEMORY",
            format!("项目记忆字段 {field} 无效。"),
            false,
        ));
    }
    Ok(value.trim().to_owned())
}

#[tauri::command]
pub fn get_project_memory(state: tauri::State<'_, AppState>) -> ApiResult<Option<ProjectMemory>> {
    state
        .project_memory
        .load_for_project(state.project_root().as_deref())
}

#[tauri::command]
pub fn save_project_memory(
    input: ProjectMemoryInput,
    state: tauri::State<'_, AppState>,
) -> ApiResult<ProjectMemory> {
    let project = state.project_root().ok_or_else(|| {
        ApiError::new(
            "PROJECT_REQUIRED",
            "请先选择项目目录，再保存项目记忆。",
            true,
        )
    })?;
    state.project_memory.save_for_project(&project, input)
}

#[tauri::command]
pub async fn run_environment_check(
    state: tauri::State<'_, AppState>,
) -> ApiResult<EnvironmentReport> {
    build_environment_report(&state).await
}

pub async fn build_environment_report(state: &AppState) -> ApiResult<EnvironmentReport> {
    let mut checks = Vec::new();
    let (claude_status, claude_summary) = check_claude();
    checks.push(EnvironmentCheck {
        id: "claude".into(),
        label: "Claude Code".into(),
        status: claude_status,
        summary: claude_summary,
        detail: "检查官方 Claude CLI 是否可以找到并执行。".into(),
        fix_available: false,
    });

    let project = state.project_root();
    let project_ok = project.as_deref().is_some_and(is_safe_directory);
    checks.push(EnvironmentCheck {
        id: "project".into(),
        label: "项目目录".into(),
        status: if project_ok { "ok" } else { "error" }.into(),
        summary: if project_ok {
            "项目目录可访问。".into()
        } else {
            "还没有可访问的项目目录。".into()
        },
        detail: project.as_ref().map_or_else(
            || "请选择一个项目文件夹。".into(),
            |path| path.to_string_lossy().into_owned(),
        ),
        fix_available: true,
    });

    let model = state.settings.model_status(project.as_deref())?;
    let model_ok = !model.candidates.is_empty() || model.desired_user_model.is_some();
    checks.push(EnvironmentCheck {
        id: "model".into(),
        label: "默认模型".into(),
        status: if model_ok { "ok" } else { "warning" }.into(),
        summary: if model_ok {
            "已找到可用模型设置。".into()
        } else {
            "还没有明确的模型设置。".into()
        },
        detail: model
            .warnings
            .first()
            .cloned()
            .unwrap_or_else(|| "可在模型栏选择或配置默认模型。".into()),
        fix_available: true,
    });

    let git = command_version("git", &["--version"]);
    checks.push(EnvironmentCheck {
        id: "git".into(),
        label: "Git".into(),
        status: if git.is_some() { "ok" } else { "warning" }.into(),
        summary: if git.is_some() {
            "Git 可用。".into()
        } else {
            "未检测到 Git，部分项目操作可能受限。".into()
        },
        detail: git.unwrap_or_else(|| "请安装 Git 后重新检查。".into()),
        fix_available: false,
    });

    let ollama = crate::commands::build_ollama_status(state).await;
    checks.push(EnvironmentCheck {
        id: "ollama".into(),
        label: "本地 Prompt 优化".into(),
        status: if ollama.online && !ollama.models.is_empty() {
            "ok"
        } else {
            "warning"
        }
        .into(),
        summary: if ollama.online {
            ollama.message.clone()
        } else {
            "Ollama 当前不可用，不影响在线 Claude 对话。".into()
        },
        detail: ollama.base_url,
        fix_available: true,
    });

    let skills = crate::commands::build_skill_inventory(state).await?;
    checks.push(EnvironmentCheck {
        id: "skills".into(),
        label: "Skills".into(),
        status: if skills.plugin_warning.is_some() {
            "warning"
        } else {
            "ok"
        }
        .into(),
        summary: format!("已扫描 {} 个 Skill。", skills.skills.len()),
        detail: skills
            .plugin_warning
            .unwrap_or_else(|| "Skill 清单可用。".into()),
        fix_available: true,
    });
    Ok(EnvironmentReport {
        checked_at_ms: now_ms(),
        checks,
    })
}

#[tauri::command]
pub async fn repair_environment_check(
    check_id: String,
    state: tauri::State<'_, AppState>,
) -> ApiResult<EnvironmentReport> {
    match check_id.as_str() {
        "skills" | "ollama" => build_environment_report(&state).await,
        "project" => Err(ApiError::new(
            "PROJECT_REQUIRED",
            "请使用项目目录选择器选择文件夹。",
            true,
        )),
        "model" => Err(ApiError::new(
            "MODEL_CONFIGURATION_REQUIRED",
            "请在模型栏选择或添加默认模型。",
            true,
        )),
        "claude" => Err(ApiError::new(
            "CLAUDE_INSTALL_REQUIRED",
            "请安装 Claude Code CLI 后重新检查。",
            true,
        )),
        "git" => Err(ApiError::new(
            "GIT_INSTALL_REQUIRED",
            "请安装 Git 后重新检查。",
            true,
        )),
        _ => Err(ApiError::new(
            "ENVIRONMENT_CHECK_NOT_FOUND",
            "找不到这个自检项目。",
            false,
        )),
    }
}

fn check_claude() -> (String, String) {
    let Some(executable) = resolve_claude_executable() else {
        return ("error".into(), "未检测到 Claude Code CLI。".into());
    };
    command_output(executable, &["--version"]).map_or_else(
        || {
            (
                "error".into(),
                "找到了 Claude CLI，但无法执行版本检查。".into(),
            )
        },
        |version| ("ok".into(), version),
    )
}

fn command_version(program: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}

fn command_output(program: PathBuf, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    if !output.status.success() || output.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: Option<String>,
    html_url: Option<String>,
    assets: Vec<ReleaseAsset>,
}
#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[tauri::command]
pub async fn check_for_updates() -> ApiResult<UpdateInfo> {
    let client = reqwest::Client::builder()
        .user_agent(format!("cc-panel/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(8))
        .redirect(Policy::limited(3))
        .build()
        .map_err(|_| ApiError::new("UPDATE_CLIENT_FAILED", "无法初始化更新检查。", true))?;
    let response = client.get(UPDATE_API).send().await.map_err(|_| {
        ApiError::new(
            "UPDATE_CHECK_FAILED",
            "暂时无法连接更新服务，请稍后重试。",
            true,
        )
    })?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            "UPDATE_CHECK_FAILED",
            "更新服务暂时不可用。",
            true,
        ));
    }
    let release: LatestRelease = response
        .json()
        .await
        .map_err(|_| ApiError::new("UPDATE_RESPONSE_INVALID", "更新信息格式无效。", true))?;
    let latest = release
        .tag_name
        .map(|tag| tag.trim_start_matches('v').to_owned());
    let installer_url = release
        .assets
        .into_iter()
        .filter(|asset| asset.name.ends_with("x64-setup.exe") || asset.name.ends_with("x64.msi"))
        .min_by_key(|asset| {
            if asset.name.ends_with("x64-setup.exe") {
                0
            } else {
                1
            }
        })
        .map(|asset| asset.browser_download_url);
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let available = latest
        .as_deref()
        .is_some_and(|version| compare_versions(version, &current).is_gt());
    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest.clone(),
        update_available: available,
        release_url: release.html_url,
        installer_url,
        message: if available {
            format!("发现新版本 {}。", latest.unwrap_or_default())
        } else {
            "当前已是最新版本。".into()
        },
    })
}

#[tauri::command]
pub async fn download_update(
    installer_url: String,
    version: String,
) -> ApiResult<DownloadedUpdate> {
    if !is_allowed_update_url(&installer_url) {
        return Err(ApiError::new(
            "UPDATE_URL_REJECTED",
            "更新下载地址不在可信来源内。",
            false,
        ));
    }
    let client = reqwest::Client::builder()
        .user_agent(format!("cc-panel/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .redirect(Policy::limited(3))
        .build()
        .map_err(|_| ApiError::new("UPDATE_CLIENT_FAILED", "无法初始化更新下载。", true))?;
    let response =
        client.get(&installer_url).send().await.map_err(|_| {
            ApiError::new("UPDATE_DOWNLOAD_FAILED", "下载更新失败，请稍后重试。", true)
        })?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            "UPDATE_DOWNLOAD_FAILED",
            "更新文件下载失败。",
            true,
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > UPDATE_DOWNLOAD_LIMIT as u64)
    {
        return Err(ApiError::new(
            "UPDATE_TOO_LARGE",
            "更新文件超过安全大小。",
            false,
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| ApiError::new("UPDATE_DOWNLOAD_FAILED", "读取更新文件失败。", true))?;
    if bytes.len() > UPDATE_DOWNLOAD_LIMIT {
        return Err(ApiError::new(
            "UPDATE_TOO_LARGE",
            "更新文件超过安全大小。",
            false,
        ));
    }
    let extension = if installer_url.to_ascii_lowercase().contains(".msi") {
        "msi"
    } else {
        "exe"
    };
    let path = std::env::temp_dir().join(format!(
        "cc-panel-update-{}.{}",
        sanitize_version(&version),
        extension
    ));
    fs::write(&path, &bytes).map_err(|_| ApiError::io("write-update"))?;
    Ok(DownloadedUpdate {
        path: path.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
    })
}

#[tauri::command]
pub fn launch_update(path: String) -> ApiResult<()> {
    let candidate = PathBuf::from(path);
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| ApiError::io("inspect-update"))?;
    let canonical = candidate
        .canonicalize()
        .map_err(|_| ApiError::new("UPDATE_NOT_FOUND", "找不到已下载的更新文件。", true))?;
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !canonical.starts_with(&temp)
        || !matches!(extension.to_ascii_lowercase().as_str(), "exe" | "msi")
    {
        return Err(ApiError::new(
            "UPDATE_PATH_REJECTED",
            "更新文件路径不安全。",
            false,
        ));
    }
    if extension.eq_ignore_ascii_case("msi") {
        Command::new("msiexec")
            .args(["/i", canonical.to_string_lossy().as_ref()])
            .spawn()
            .map_err(|_| ApiError::new("UPDATE_LAUNCH_FAILED", "无法启动安装程序。", true))?;
    } else {
        let mut command = Command::new(&canonical);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
            .spawn()
            .map_err(|_| ApiError::new("UPDATE_LAUNCH_FAILED", "无法启动安装程序。", true))?;
    }
    Ok(())
}

#[tauri::command]
pub fn collect_diagnostics(state: tauri::State<'_, AppState>) -> ApiResult<DiagnosticResult> {
    let root = state.paths.home().join(".cc-panel").join("diagnostics");
    fs::create_dir_all(&root).map_err(|_| ApiError::io("create-diagnostics"))?;
    let report = serde_json::json!({
        "appVersion": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "projectPath": state.project_root().map(|path| path.to_string_lossy().into_owned()),
        "claudeVersion": check_claude().1,
        "conversationCount": state.conversations.list().len(),
        "modelProfiles": state.model_profiles.list()?.profiles.iter().map(|profile| serde_json::json!({
            "providerName": profile.provider_name, "modelId": profile.model_id,
            "selected": profile.selected, "hasApiKey": profile.has_api_key,
        })).collect::<Vec<_>>(),
        "skillInventory": state.skill_inventory.read().ok().and_then(|value| value.as_ref().map(|inventory| serde_json::json!({
            "count": inventory.skills.len(), "scannedAtRevision": inventory.scanned_at_revision,
            "claudeCliAvailable": inventory.claude_cli_available,
        }))),
        "settingsRevision": state.settings.settings_revision().ok(),
    });
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|_| ApiError::new("DIAGNOSTIC_SERIALIZATION_FAILED", "无法生成诊断包。", false))?;
    if bytes.len() > DIAGNOSTIC_LIMIT {
        return Err(ApiError::new(
            "DIAGNOSTIC_TOO_LARGE",
            "诊断信息超过安全大小。",
            false,
        ));
    }
    let created_at_ms = now_ms();
    let path = root.join(format!("cc-panel-diagnostic-{}.json", created_at_ms));
    replace_file_atomically(&path, &bytes)?;
    Ok(DiagnosticResult {
        path: path.to_string_lossy().into_owned(),
        created_at_ms,
        included_sections: vec![
            "environment".into(),
            "configuration-summary".into(),
            "runtime-summary".into(),
        ],
    })
}

fn is_safe_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn is_allowed_update_url(value: &str) -> bool {
    url::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https"
            && match url.host_str() {
                Some("github.com") => url.path().contains("/VIPmrgj/cc-panel/"),
                Some("objects.githubusercontent.com") => true,
                _ => false,
            }
    })
}

fn sanitize_version(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
        .collect();
    if sanitized.is_empty() {
        "latest".into()
    } else {
        sanitized
    }
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let a = parse(left);
    let b = parse(right);
    for index in 0..a.len().max(b.len()) {
        match a.get(index).unwrap_or(&0).cmp(b.get(index).unwrap_or(&0)) {
            std::cmp::Ordering::Equal => {}
            order => return order,
        }
    }
    std::cmp::Ordering::Equal
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_memory_round_trip_supports_multiline_rules() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProjectMemoryStore::new(temp.path().join("memory")).unwrap();
        let project = Path::new("C:\\workspace\\cc-panel");
        let saved = store
            .save_for_project(
                project,
                ProjectMemoryInput {
                    enabled: true,
                    purpose: "desktop agent".into(),
                    tech_stack: "React + Rust".into(),
                    rules: "先检查\n再修改".into(),
                    avoid: "不要删除测试".into(),
                    test_command: "npm test".into(),
                    preferred_language: "中文".into(),
                },
            )
            .unwrap();

        let loaded = store.load_for_project(Some(project)).unwrap().unwrap();
        assert_eq!(saved.rules, loaded.rules);
        assert!(loaded.enabled);
    }
}
