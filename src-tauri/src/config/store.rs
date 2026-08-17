use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dto::{ApiError, ApiResult, AppPreferences, RootEntry, RootKind};
use crate::platform::replace_file_atomically;

const CONFIG_FILE_LIMIT: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredConfig {
    schema_version: u32,
    selected_project_root: Option<RootEntry>,
    additional_roots: Vec<RootEntry>,
    ollama: StoredOllama,
    native_notifications_enabled: bool,
}

impl Default for StoredConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            selected_project_root: None,
            additional_roots: Vec::new(),
            ollama: StoredOllama::default(),
            native_notifications_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredOllama {
    base_url: String,
    model: Option<String>,
}

impl Default for StoredOllama {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            model: None,
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    value: Mutex<StoredConfig>,
}

impl ConfigStore {
    pub fn load(path: PathBuf) -> ApiResult<Self> {
        let value = if path.exists() {
            let metadata = fs::metadata(&path).map_err(|_| ApiError::io("read-app-config"))?;
            if metadata.len() > CONFIG_FILE_LIMIT {
                return Err(ApiError::new(
                    "APP_CONFIG_TOO_LARGE",
                    "CC Panel 配置文件异常过大。",
                    false,
                ));
            }
            let bytes = fs::read(&path).map_err(|_| ApiError::io("read-app-config"))?;
            serde_json::from_slice(&bytes).map_err(|_| {
                ApiError::new(
                    "INVALID_APP_CONFIG",
                    "CC Panel 配置文件无效。请修复或移除该文件后重试。",
                    false,
                )
            })?
        } else {
            StoredConfig::default()
        };
        Ok(Self {
            path,
            value: Mutex::new(value),
        })
    }

    pub fn preferences(&self) -> AppPreferences {
        let value = self.value.lock().expect("config mutex poisoned");
        AppPreferences {
            selected_project_root: value.selected_project_root.clone(),
            additional_roots: value.additional_roots.clone(),
            ollama_base_url: value.ollama.base_url.clone(),
            ollama_model: value.ollama.model.clone(),
            native_notifications_enabled: value.native_notifications_enabled,
        }
    }

    pub fn set_project_root(&self, path: Option<&Path>) -> ApiResult<Option<RootEntry>> {
        let entry = path.map(|path| make_root_entry(path, RootKind::Project));
        let mut value = self.value.lock().expect("config mutex poisoned");
        let mut next = value.clone();
        next.selected_project_root = entry.clone();
        self.save_locked(&next)?;
        *value = next;
        Ok(entry)
    }

    pub fn add_additional_root(&self, path: &Path) -> ApiResult<RootEntry> {
        let entry = make_root_entry(path, RootKind::Additional);
        let mut value = self.value.lock().expect("config mutex poisoned");
        if let Some(existing) = value
            .additional_roots
            .iter()
            .find(|root| root.path.eq_ignore_ascii_case(&entry.path))
        {
            return Ok(existing.clone());
        }
        let mut next = value.clone();
        next.additional_roots.push(entry.clone());
        self.save_locked(&next)?;
        *value = next;
        Ok(entry)
    }

    pub fn remove_additional_root(&self, id: &str) -> ApiResult<()> {
        let mut value = self.value.lock().expect("config mutex poisoned");
        let mut next = value.clone();
        next.additional_roots.retain(|root| root.id != id);
        self.save_locked(&next)?;
        *value = next;
        Ok(())
    }

    pub fn set_ollama(&self, base_url: String, model: Option<String>) -> ApiResult<()> {
        let mut value = self.value.lock().expect("config mutex poisoned");
        let mut next = value.clone();
        next.ollama = StoredOllama { base_url, model };
        self.save_locked(&next)?;
        *value = next;
        Ok(())
    }

    pub fn set_notifications(&self, enabled: bool) -> ApiResult<()> {
        let mut value = self.value.lock().expect("config mutex poisoned");
        let mut next = value.clone();
        next.native_notifications_enabled = enabled;
        self.save_locked(&next)?;
        *value = next;
        Ok(())
    }

    fn save_locked(&self, value: &StoredConfig) -> ApiResult<()> {
        let bytes = serde_json::to_vec_pretty(value).map_err(|_| {
            ApiError::new(
                "APP_CONFIG_SERIALIZATION_FAILED",
                "无法保存 CC Panel 配置。",
                false,
            )
        })?;
        if bytes.len() > CONFIG_FILE_LIMIT as usize {
            return Err(ApiError::new(
                "APP_CONFIG_TOO_LARGE",
                "CC Panel 配置文件异常过大。",
                false,
            ));
        }
        replace_file_atomically(&self.path, &bytes)
    }
}

fn make_root_entry(path: &Path, kind: RootKind) -> RootEntry {
    let path_string = path.to_string_lossy().into_owned();
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&path_string)
        .to_owned();
    let mut digest = Sha256::new();
    digest.update(path_string.as_bytes());
    let stable_hash = hex::encode(digest.finalize());
    RootEntry {
        id: format!(
            "{}-{}",
            match kind {
                RootKind::Project => "project",
                RootKind::Additional => "additional",
            },
            &stable_hash[..16]
        ),
        path: path_string,
        label,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_only_non_sensitive_preferences() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(temp.path().join("cc-panel.json")).unwrap();
        let root = temp.path().join("workspace");
        fs::create_dir(&root).unwrap();
        store.set_project_root(Some(&root)).unwrap();
        store
            .set_ollama("http://localhost:11434".into(), Some("qwen".into()))
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(temp.path().join("cc-panel.json")).unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert!(value.get("prompt").is_none());
        assert!(value.get("attachments").is_none());
    }
}
