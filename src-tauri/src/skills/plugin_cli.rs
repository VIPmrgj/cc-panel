use std::{path::PathBuf, process::Stdio, time::Duration};

use serde::Deserialize;
use tokio::{process::Command, time::timeout};

use crate::{
    dto::{ApiError, ApiResult},
    platform::resolve_claude_executable,
};

const PLUGIN_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PluginRoot {
    pub plugin_name: String,
    pub plugin_id: String,
    pub install_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct PluginCli;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginListEntry {
    id: String,
    enabled: bool,
    install_path: Option<String>,
}

impl PluginCli {
    pub async fn enabled_roots(&self) -> ApiResult<Vec<PluginRoot>> {
        let executable = resolve_claude_executable().ok_or_else(|| {
            ApiError::new(
                "PLUGIN_CLI_UNAVAILABLE",
                "找不到 Claude CLI，插件 Skill 暂不可用。",
                true,
            )
        })?;
        let mut command = Command::new(executable);
        command
            .args(["plugin", "list", "--json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            // tokio::process::Command has an inherent creation_flags().
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW：隐藏 Claude CLI 控制台窗口
        }
        let child = command.spawn().map_err(|_| {
            ApiError::new(
                "PLUGIN_CLI_UNAVAILABLE",
                "找不到 Claude CLI，插件 Skill 暂不可用。",
                true,
            )
        })?;

        let output = timeout(Duration::from_secs(8), child.wait_with_output())
            .await
            .map_err(|_| ApiError::new("PLUGIN_CLI_TIMEOUT", "Claude 插件清单查询超时。", true))?
            .map_err(|_| ApiError::new("PLUGIN_CLI_FAILED", "无法查询 Claude 插件清单。", true))?;
        if !output.status.success() {
            return Err(ApiError::new(
                "PLUGIN_CLI_FAILED",
                "Claude CLI 未能返回插件清单。",
                true,
            ));
        }
        if output.stdout.len() > PLUGIN_OUTPUT_LIMIT {
            return Err(ApiError::new(
                "PLUGIN_OUTPUT_TOO_LARGE",
                "Claude 插件清单异常过大。",
                false,
            ));
        }
        let entries: Vec<PluginListEntry> =
            serde_json::from_slice(&output.stdout).map_err(|_| {
                ApiError::new(
                    "PLUGIN_OUTPUT_INVALID",
                    "Claude CLI 插件清单格式无法识别。",
                    true,
                )
            })?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.enabled)
            .filter_map(|entry| {
                let path = entry.install_path?;
                let plugin_name = entry.id.split('@').next()?.to_owned();
                Some(PluginRoot {
                    plugin_name,
                    plugin_id: entry.id,
                    install_path: PathBuf::from(path),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_schema_requires_explicit_enabled() {
        let entries: Vec<PluginListEntry> = serde_json::from_str(
            r#"[{"id":"a@m","enabled":false,"installPath":"C:/cache/a"},{"id":"b@m","enabled":true,"installPath":"C:/cache/b"}]"#,
        )
        .unwrap();
        assert_eq!(entries.iter().filter(|entry| entry.enabled).count(), 1);
    }
}
