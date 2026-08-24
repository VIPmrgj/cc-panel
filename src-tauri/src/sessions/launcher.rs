use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Stdio,
};

use thiserror::Error;
use tokio::process::{Child, Command};
use uuid::Uuid;

use crate::platform::resolve_claude_executable;

const AUTOCOMPACT_VALUE: &str = "272k";
const DEFAULT_LANGUAGE_SYSTEM_PROMPT: &str = "默认使用简体中文与用户交流。进度、权限请求、错误说明和最终总结使用中文。代码、命令、文件路径、API 名称和专有名词保持原文。用户明确要求其他语言时，遵循用户要求。";

/// Win32 `CREATE_NO_WINDOW`: keeps the spawned Claude CLI console window
/// invisible on Windows. std exposes no getter for creation flags, so the
/// constant is shared with tests as the observable contract.
#[cfg(windows)]
const WINDOWS_SPAWN_CREATION_FLAGS: u32 = 0x0800_0000;

#[cfg(windows)]
const INHERITED_ENV_ALLOWLIST: &[&str] = &[
    "ALLUSERSPROFILE",
    "APPDATA",
    "COMMONPROGRAMFILES",
    "COMMONPROGRAMFILES(X86)",
    "COMMONPROGRAMW6432",
    "COMSPEC",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "PATH",
    "PATHEXT",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "PUBLIC",
    "SYSTEMDRIVE",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "USERDOMAIN",
    "USERDOMAIN_ROAMINGPROFILE",
    "USERNAME",
    "USERPROFILE",
    "WINDIR",
];

#[cfg(not(windows))]
const INHERITED_ENV_ALLOWLIST: &[&str] = &[
    "COLORTERM",
    "HOME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "LOGNAME",
    "NO_COLOR",
    "PATH",
    "SHELL",
    "TERM",
    "TMPDIR",
    "TZ",
    "USER",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_RUNTIME_DIR",
    "XDG_STATE_HOME",
];

/// How the CLI should establish the conversation before it reads its first
/// NDJSON user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionMode {
    New,
    Resume {
        session_id: String,
    },
    Continue,
    Fork {
        session_id: String,
    },
    /// Claude Code has no separate `--retry` process flag. A retry is a
    /// resume of the existing conversation; the manager sends the replacement
    /// user turn through the normal stream-json input channel.
    Retry {
        session_id: String,
    },
}

impl SessionMode {
    fn validate(&self) -> Result<(), LaunchError> {
        match self {
            Self::New | Self::Continue => Ok(()),
            Self::Resume { session_id }
            | Self::Fork { session_id }
            | Self::Retry { session_id } => validate_session_id(session_id),
        }
    }
}

/// Provider credentials are held separately from the inherited environment.
/// They are applied to the child only at spawn time and are never included in
/// `Debug`, event payloads, or command arguments.
#[derive(Clone, Default)]
pub struct ProviderSecrets {
    anthropic_auth_token: Option<OsString>,
    anthropic_base_url: Option<OsString>,
    anthropic_model: Option<OsString>,
}

impl ProviderSecrets {
    fn new(
        anthropic_auth_token: Option<OsString>,
        anthropic_base_url: Option<OsString>,
        anthropic_model: Option<OsString>,
    ) -> Self {
        Self {
            anthropic_auth_token,
            anthropic_base_url,
            anthropic_model,
        }
    }

    /// Builds the environment contract for an Anthropic-compatible provider.
    /// The profile key is treated as an auth token and never becomes a CLI arg.
    pub fn anthropic_compatible(
        api_key: impl Into<OsString>,
        base_url: impl Into<OsString>,
        model: impl Into<OsString>,
    ) -> Self {
        Self::new(
            Some(api_key.into()),
            Some(base_url.into()),
            Some(model.into()),
        )
    }

    fn apply_to(&self, command: &mut Command) {
        if let Some(value) = &self.anthropic_auth_token {
            command.env("ANTHROPIC_AUTH_TOKEN", value);
        }
        if let Some(value) = &self.anthropic_base_url {
            command.env("ANTHROPIC_BASE_URL", value);
        }
        if let Some(value) = &self.anthropic_model {
            command.env("ANTHROPIC_MODEL", value);
        }
    }
}

impl std::fmt::Debug for ProviderSecrets {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderSecrets")
            .field(
                "anthropic_auth_token",
                &self.anthropic_auth_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "anthropic_base_url",
                &self.anthropic_base_url.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "anthropic_model",
                &self.anthropic_model.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct LaunchOptions {
    pub mode: SessionMode,
    pub session_id: Option<String>,
    /// Optional explicit model override, primarily useful when forking a
    /// session whose provider profile must not be changed.
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub add_dirs: Vec<PathBuf>,
    pub include_partial_messages: bool,
    pub include_hook_events: bool,
    pub provider_secrets: ProviderSecrets,
}

impl LaunchOptions {
    pub fn new(mode: SessionMode) -> Self {
        Self {
            mode,
            session_id: None,
            model: None,
            cwd: None,
            add_dirs: Vec::new(),
            include_partial_messages: true,
            include_hook_events: false,
            provider_secrets: ProviderSecrets::default(),
        }
    }
}

impl std::fmt::Debug for LaunchOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LaunchOptions")
            .field("mode", &self.mode)
            .field("session_id", &self.session_id)
            .field("model", &self.model)
            .field("cwd", &self.cwd)
            .field("add_dirs", &self.add_dirs)
            .field("include_partial_messages", &self.include_partial_messages)
            .field("include_hook_events", &self.include_hook_events)
            .field("provider_secrets", &self.provider_secrets)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeLauncher {
    executable: PathBuf,
}

impl Default for ClaudeLauncher {
    fn default() -> Self {
        Self::new(resolve_claude_executable().unwrap_or_else(|| PathBuf::from("claude")))
    }
}

impl ClaudeLauncher {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the exact argv vector passed to the official CLI. No prompt,
    /// credential, cwd, or add-dir value is concatenated into a shell string.
    pub fn args(&self, options: &LaunchOptions) -> Result<Vec<OsString>, LaunchError> {
        options.mode.validate()?;
        if let Some(session_id) = &options.session_id {
            validate_session_id(session_id)?;
        }
        let mut args = vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("stream-json"),
            OsString::from("--input-format"),
            OsString::from("stream-json"),
            OsString::from("--verbose"),
            OsString::from("--permission-prompt-tool"),
            OsString::from("stdio"),
            OsString::from("--autocompact"),
            OsString::from(AUTOCOMPACT_VALUE),
            OsString::from("--append-system-prompt"),
            OsString::from(DEFAULT_LANGUAGE_SYSTEM_PROMPT),
        ];

        if options.include_partial_messages {
            args.push(OsString::from("--include-partial-messages"));
        }
        if options.include_hook_events {
            args.push(OsString::from("--include-hook-events"));
        }
        if let Some(model) = &options.model {
            if model.is_empty() || model.len() > 256 || model.contains(['\r', '\n', '\0']) {
                return Err(LaunchError::InvalidModel);
            }
            args.extend([OsString::from("--model"), OsString::from(model)]);
        }
        for directory in &options.add_dirs {
            args.push(OsString::from("--add-dir"));
            args.push(directory.as_os_str().to_owned());
        }

        match &options.mode {
            SessionMode::New => {
                let session_id = options
                    .session_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                args.extend([OsString::from("--session-id"), OsString::from(session_id)]);
            }
            SessionMode::Resume { session_id } => {
                args.extend([OsString::from("--resume"), OsString::from(session_id)]);
            }
            SessionMode::Continue => args.push(OsString::from("--continue")),
            SessionMode::Fork { session_id } => {
                args.extend([
                    OsString::from("--resume"),
                    OsString::from(session_id),
                    OsString::from("--fork-session"),
                ]);
            }
            SessionMode::Retry { session_id } => {
                args.extend([OsString::from("--resume"), OsString::from(session_id)]);
            }
        }
        Ok(args)
    }

    pub fn prepare_command(&self, options: &LaunchOptions) -> Result<Command, LaunchError> {
        let args = self.args(options)?;
        if let Some(cwd) = &options.cwd {
            validate_spawn_directory(cwd)?;
        }
        for directory in &options.add_dirs {
            validate_spawn_directory(directory)?;
        }
        let mut command = Command::new(&self.executable);
        command.args(args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);
        #[cfg(windows)]
        {
            // tokio::process::Command has an inherent creation_flags().
            command.creation_flags(WINDOWS_SPAWN_CREATION_FLAGS);
        }
        if let Some(cwd) = &options.cwd {
            command.current_dir(cwd);
        }
        sanitize_environment(&mut command);
        options.provider_secrets.apply_to(&mut command);
        Ok(command)
    }

    pub fn spawn(&self, options: &LaunchOptions) -> Result<Child, LaunchError> {
        let mut command = self.prepare_command(options)?;
        command.spawn().map_err(LaunchError::Spawn)
    }
}

fn sanitize_environment(command: &mut Command) {
    sanitize_environment_from(command, std::env::vars_os());
}

fn sanitize_environment_from(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) {
    command.env_clear();
    for (key, value) in inherited {
        if inherited_environment_key_allowed(&key) {
            command.env(key, value);
        }
    }
}

fn inherited_environment_key_allowed(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    #[cfg(windows)]
    return INHERITED_ENV_ALLOWLIST
        .iter()
        .any(|allowed| key.eq_ignore_ascii_case(allowed));
    #[cfg(not(windows))]
    return INHERITED_ENV_ALLOWLIST.contains(&key);
}

fn validate_session_id(session_id: &str) -> Result<(), LaunchError> {
    if session_id.is_empty()
        || session_id.len() > 256
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(LaunchError::InvalidSessionId);
    }
    Ok(())
}

fn validate_spawn_directory(path: &Path) -> Result<(), LaunchError> {
    let metadata = std::fs::symlink_metadata(path).map_err(LaunchError::UnsafeDirectory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(LaunchError::InvalidDirectory);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("session id is invalid")]
    InvalidSessionId,
    #[error("model identifier is invalid")]
    InvalidModel,
    #[error("the launch directory is not a safe ordinary directory")]
    InvalidDirectory,
    #[error("failed to inspect a launch directory")]
    UnsafeDirectory(#[source] std::io::Error),
    #[error("failed to launch Claude CLI")]
    Spawn(#[source] std::io::Error),
    #[cfg(windows)]
    #[error("failed to place Claude CLI in a kill-on-close job")]
    JobObject(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_environment_is_a_positive_allowlist() {
        let mut command = Command::new("claude");
        let allowed_key = if cfg!(windows) { "SystemRoot" } else { "HOME" };
        sanitize_environment_from(
            &mut command,
            [
                (OsString::from(allowed_key), OsString::from("safe-value")),
                (
                    OsString::from("ANTHROPIC_AUTH_TOKEN"),
                    OsString::from("provider-secret"),
                ),
                (
                    OsString::from("VSCODE_GIT_ASKPASS_MAIN"),
                    OsString::from("editor-secret"),
                ),
                (
                    OsString::from("AWS_SECRET_ACCESS_KEY"),
                    OsString::from("cloud-secret"),
                ),
                (
                    OsString::from("GITHUB_TOKEN"),
                    OsString::from("source-control-secret"),
                ),
                (
                    OsString::from("HTTPS_PROXY"),
                    OsString::from("http://name:password@example.test"),
                ),
            ],
        );

        let environment = command
            .as_std()
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            environment.get(OsStr::new(allowed_key)),
            Some(&Some(OsString::from("safe-value")))
        );
        for denied in [
            "ANTHROPIC_AUTH_TOKEN",
            "VSCODE_GIT_ASKPASS_MAIN",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "HTTPS_PROXY",
        ] {
            assert!(!environment.contains_key(OsStr::new(denied)), "{denied}");
        }
    }

    #[test]
    fn provider_environment_contains_only_the_three_supported_anthropic_keys() {
        let mut command = Command::new("claude");
        sanitize_environment_from(&mut command, std::iter::empty());
        ProviderSecrets::anthropic_compatible(
            "sk-ant-test-secret",
            "https://provider.example",
            "claude-opus-5",
        )
        .apply_to(&mut command);

        let environment = command
            .as_std()
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(environment.len(), 3);
        assert_eq!(
            environment.get(OsStr::new("ANTHROPIC_AUTH_TOKEN")),
            Some(&OsString::from("sk-ant-test-secret"))
        );
        assert_eq!(
            environment.get(OsStr::new("ANTHROPIC_BASE_URL")),
            Some(&OsString::from("https://provider.example"))
        );
        assert_eq!(
            environment.get(OsStr::new("ANTHROPIC_MODEL")),
            Some(&OsString::from("claude-opus-5"))
        );
        assert!(!environment.contains_key(OsStr::new("ANTHROPIC_API_KEY")));
    }

    #[test]
    fn provider_secret_debug_is_redacted() {
        let secrets = ProviderSecrets::anthropic_compatible(
            "sk-ant-test-secret",
            "https://provider.example",
            "claude-opus-5",
        );
        let debug = format!("{secrets:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("sk-ant-test-secret"));
    }

    #[test]
    fn every_mode_has_one_autocompact_pair_and_stream_flags() {
        let modes = [
            SessionMode::New,
            SessionMode::Resume {
                session_id: "session-1".into(),
            },
            SessionMode::Continue,
            SessionMode::Fork {
                session_id: "session-1".into(),
            },
            SessionMode::Retry {
                session_id: "session-1".into(),
            },
        ];
        for mode in modes {
            let options = LaunchOptions::new(mode);
            let args = ClaudeLauncher::default().args(&options).unwrap();
            assert_eq!(
                args.iter()
                    .filter(|arg| arg.as_os_str() == std::ffi::OsStr::new("--autocompact"))
                    .count(),
                1
            );
            assert_eq!(
                args.iter()
                    .filter(|arg| arg.as_os_str() == std::ffi::OsStr::new("272k"))
                    .count(),
                1
            );
            assert!(args.windows(2).any(|pair| {
                pair[0].as_os_str() == std::ffi::OsStr::new("--output-format")
                    && pair[1].as_os_str() == std::ffi::OsStr::new("stream-json")
            }));
            assert!(args.windows(2).any(|pair| {
                pair[0].as_os_str() == std::ffi::OsStr::new("--input-format")
                    && pair[1].as_os_str() == std::ffi::OsStr::new("stream-json")
            }));
            assert!(args.windows(2).any(|pair| {
                pair[0].as_os_str() == std::ffi::OsStr::new("--permission-prompt-tool")
                    && pair[1].as_os_str() == std::ffi::OsStr::new("stdio")
            }));
            assert!(args.windows(2).any(|pair| {
                pair[0].as_os_str() == std::ffi::OsStr::new("--append-system-prompt")
                    && pair[1].as_os_str() == std::ffi::OsStr::new(DEFAULT_LANGUAGE_SYSTEM_PROMPT)
            }));
        }
    }

    #[test]
    fn fork_is_resume_plus_fork_without_shell_construction() {
        let options = LaunchOptions {
            mode: SessionMode::Fork {
                session_id: "abc_123".into(),
            },
            session_id: None,
            model: Some("claude-opus-5".into()),
            cwd: Some(PathBuf::from(r"C:\safe path; not shell")),
            add_dirs: vec![PathBuf::from(r"C:\another path")],
            include_partial_messages: true,
            include_hook_events: true,
            provider_secrets: ProviderSecrets::default(),
        };
        let args = ClaudeLauncher::default().args(&options).unwrap();
        assert!(args.windows(2).any(|pair| {
            pair[0].as_os_str() == std::ffi::OsStr::new("--resume")
                && pair[1].as_os_str() == std::ffi::OsStr::new("abc_123")
        }));
        assert!(args.contains(&OsString::from("--fork-session")));
        assert_eq!(args.iter().filter(|arg| *arg == "--add-dir").count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn session_spawn_applies_create_no_window() {
        // std exposes no getter for creation flags; the shared constant is the
        // observable contract between prepare_command and this test.
        assert_eq!(WINDOWS_SPAWN_CREATION_FLAGS, 0x0800_0000);
        let options = LaunchOptions::new(SessionMode::New);
        assert!(ClaudeLauncher::default().prepare_command(&options).is_ok());
    }

    #[test]
    fn invalid_ids_are_rejected_before_spawn() {
        let options = LaunchOptions::new(SessionMode::Resume {
            session_id: "id with spaces".into(),
        });
        assert!(matches!(
            ClaudeLauncher::default().args(&options),
            Err(LaunchError::InvalidSessionId)
        ));
    }

    #[test]
    fn symlinked_launch_directories_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        let link = temp.path().join("link");
        std::fs::create_dir(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(windows)]
        if let Err(error) = std::os::windows::fs::symlink_dir(&real, &link) {
            if error.raw_os_error() == Some(1314) {
                return;
            }
            panic!("failed to create test symlink: {error}");
        }

        let mut options = LaunchOptions::new(SessionMode::Continue);
        options.cwd = Some(link);
        assert!(matches!(
            ClaudeLauncher::default().prepare_command(&options),
            Err(LaunchError::InvalidDirectory)
        ));
    }
}
