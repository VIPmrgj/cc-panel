use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use fs2::FileExt;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    dto::{ApiError, ApiResult},
    platform::replace_file_atomically,
};

const SETTINGS_FILE_LIMIT: u64 = 4 * 1024 * 1024;

pub struct SettingsDocument {
    pub object: Map<String, Value>,
    pub revision: String,
}

#[derive(Clone)]
pub struct SettingsTransaction {
    target: PathBuf,
    lock_path: PathBuf,
}

impl SettingsTransaction {
    pub fn new(target: PathBuf, lock_path: PathBuf) -> Self {
        Self { target, lock_path }
    }

    pub fn read(&self) -> ApiResult<SettingsDocument> {
        read_document(&self.target)
    }

    pub fn update<F>(&self, expected_revision: &str, mutation: F) -> ApiResult<String>
    where
        F: FnOnce(&mut Map<String, Value>) -> ApiResult<()>,
    {
        let lock_parent = self
            .lock_path
            .parent()
            .ok_or_else(|| ApiError::new("UNSAFE_SETTINGS_PATH", "设置锁路径无效。", false))?;
        fs::create_dir_all(lock_parent)
            .map_err(|_| ApiError::io("create-settings-lock-directory"))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|_| ApiError::io("open-settings-lock"))?;
        lock.lock_exclusive()
            .map_err(|_| ApiError::io("lock-settings"))?;

        let result = self.update_locked(expected_revision, mutation);
        let _ = FileExt::unlock(&lock);
        result
    }

    fn update_locked<F>(&self, expected_revision: &str, mutation: F) -> ApiResult<String>
    where
        F: FnOnce(&mut Map<String, Value>) -> ApiResult<()>,
    {
        validate_target(&self.target)?;
        let mut document = read_document(&self.target)?;
        if document.revision != expected_revision {
            return Err(ApiError::settings_conflict());
        }

        mutation(&mut document.object)?;
        let bytes = serde_json::to_vec_pretty(&Value::Object(document.object)).map_err(|_| {
            ApiError::new(
                "SETTINGS_SERIALIZATION_FAILED",
                "无法序列化 Claude Code 设置；未做任何修改。",
                false,
            )
        })?;

        let before_replace = read_document(&self.target)?;
        if before_replace.revision != expected_revision {
            return Err(ApiError::settings_conflict());
        }

        replace_file_atomically(&self.target, &bytes)?;
        Ok(revision_for(&bytes))
    }
}

fn validate_target(target: &Path) -> ApiResult<()> {
    let parent = target.parent().ok_or_else(|| {
        ApiError::new("UNSAFE_SETTINGS_PATH", "Claude Code 设置路径无效。", false)
    })?;
    if target.exists() {
        let metadata =
            fs::symlink_metadata(target).map_err(|_| ApiError::io("inspect-settings"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ApiError::new(
                "UNSAFE_SETTINGS_PATH",
                "拒绝写入符号链接或非普通 settings.json。",
                false,
            ));
        }
    }
    if parent.exists() {
        let metadata =
            fs::symlink_metadata(parent).map_err(|_| ApiError::io("inspect-settings-directory"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ApiError::new(
                "UNSAFE_SETTINGS_PATH",
                "Claude Code 设置目录不是安全的普通目录。",
                false,
            ));
        }
    }
    Ok(())
}

fn read_document(path: &Path) -> ApiResult<SettingsDocument> {
    if !path.exists() {
        return Ok(SettingsDocument {
            object: Map::new(),
            revision: revision_for(&[]),
        });
    }
    let metadata = fs::metadata(path).map_err(|_| ApiError::io("read-settings-metadata"))?;
    if metadata.len() > SETTINGS_FILE_LIMIT {
        return Err(ApiError::new(
            "SETTINGS_FILE_TOO_LARGE",
            "Claude Code settings.json 异常过大；未做任何修改。",
            false,
        ));
    }
    let file = File::open(path).map_err(|_| ApiError::io("open-settings"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(SETTINGS_FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ApiError::io("read-settings"))?;
    if bytes.len() as u64 > SETTINGS_FILE_LIMIT {
        return Err(ApiError::new(
            "SETTINGS_FILE_TOO_LARGE",
            "Claude Code settings.json 异常过大；未做任何修改。",
            false,
        ));
    }
    let object = match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(object)) => object,
        _ => return Err(ApiError::settings_invalid()),
    };
    Ok(SettingsDocument {
        object,
        revision: revision_for(&bytes),
    })
}

fn revision_for(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_and_secret_bearing_fields() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("settings.json");
        fs::write(
            &target,
            br#"{"env":{"ANTHROPIC_AUTH_TOKEN":"secret"},"future":{"x":1},"model":"old"}"#,
        )
        .unwrap();
        let transaction = SettingsTransaction::new(target.clone(), temp.path().join("lock"));
        let revision = transaction.read().unwrap().revision;
        transaction
            .update(&revision, |object| {
                object.insert("model".into(), Value::String("custom/model".into()));
                Ok(())
            })
            .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(target).unwrap()).unwrap();
        assert_eq!(value["env"]["ANTHROPIC_AUTH_TOKEN"], "secret");
        assert_eq!(value["future"]["x"], 1);
        assert_eq!(value["model"], "custom/model");
    }

    #[test]
    fn rejects_stale_revision() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("settings.json");
        fs::write(&target, b"{}").unwrap();
        let transaction = SettingsTransaction::new(target.clone(), temp.path().join("lock"));
        let revision = transaction.read().unwrap().revision;
        fs::write(&target, br#"{"model":"other"}"#).unwrap();
        let error = transaction.update(&revision, |_| Ok(())).unwrap_err();
        assert_eq!(error.code, "SETTINGS_CONFLICT");
    }
}
