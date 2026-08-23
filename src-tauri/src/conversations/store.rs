use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    dto::{ApiError, ApiResult},
    platform::replace_file_atomically,
};

const CONVERSATION_FILE_LIMIT: u64 = 512 * 1024;
const MAX_CONVERSATIONS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationStatus {
    Idle,
    Starting,
    Running,
    AwaitingPermission,
    Stopping,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMetadata {
    pub session_id: String,
    #[serde(default)]
    pub active_run_id: Option<String>,
    pub title: String,
    pub project_path: String,
    pub profile_id: Option<String>,
    pub provider_name: Option<String>,
    pub model_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub status: ConversationStatus,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone)]
pub struct UpsertConversation {
    pub session_id: String,
    pub active_run_id: Option<String>,
    pub title: String,
    pub project_path: String,
    pub profile_id: Option<String>,
    pub provider_name: Option<String>,
    pub model_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub now_ms: u64,
    pub status: ConversationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredIndex {
    schema_version: u32,
    conversations: Vec<ConversationMetadata>,
}

impl Default for StoredIndex {
    fn default() -> Self {
        Self {
            schema_version: 1,
            conversations: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct ConversationIndex {
    path: PathBuf,
    value: Arc<Mutex<StoredIndex>>,
}

impl ConversationIndex {
    pub fn load(path: PathBuf) -> ApiResult<Self> {
        let value = if path.exists() {
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| ApiError::io("read-conversation-index"))?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > CONVERSATION_FILE_LIMIT
            {
                return Err(ApiError::new(
                    "CONVERSATION_INDEX_INVALID",
                    "会话索引无效或异常过大。",
                    false,
                ));
            }
            let bytes = fs::read(&path).map_err(|_| ApiError::io("read-conversation-index"))?;
            let parsed: StoredIndex = serde_json::from_slice(&bytes).map_err(|_| {
                ApiError::new("CONVERSATION_INDEX_INVALID", "会话索引 JSON 无效。", false)
            })?;
            if parsed.schema_version != 1 || parsed.conversations.len() > MAX_CONVERSATIONS {
                return Err(ApiError::new(
                    "CONVERSATION_INDEX_UNSUPPORTED",
                    "会话索引版本或数量不受支持。",
                    false,
                ));
            }
            parsed
        } else {
            StoredIndex::default()
        };
        Ok(Self {
            path,
            value: Arc::new(Mutex::new(value)),
        })
    }

    pub fn list(&self) -> Vec<ConversationMetadata> {
        let mut entries = self
            .value
            .lock()
            .expect("conversation index poisoned")
            .conversations
            .clone();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
        entries
    }

    pub fn upsert(&self, input: UpsertConversation) -> ApiResult<ConversationMetadata> {
        validate_identifier(&input.session_id, "sessionId")?;
        validate_text(&input.title, 256, "title")?;
        validate_text(&input.project_path, 4096, "projectPath")?;
        let _lock = self.acquire_lock()?;
        let mut guard = self.value.lock().expect("conversation index poisoned");
        let mut next = self.reload_locked()?;
        let existing = next
            .conversations
            .iter()
            .find(|item| item.session_id == input.session_id)
            .cloned();
        let entry = ConversationMetadata {
            session_id: input.session_id,
            active_run_id: input.active_run_id,
            title: input.title,
            project_path: input.project_path,
            profile_id: input.profile_id,
            provider_name: input.provider_name,
            model_id: input.model_id,
            parent_session_id: input.parent_session_id,
            created_at_ms: existing
                .as_ref()
                .map_or(input.now_ms, |item| item.created_at_ms),
            updated_at_ms: input.now_ms,
            status: input.status,
            favorite: existing.as_ref().is_some_and(|item| item.favorite),
            archived: existing.as_ref().is_some_and(|item| item.archived),
        };
        if let Some(position) = next
            .conversations
            .iter()
            .position(|item| item.session_id == entry.session_id)
        {
            next.conversations[position] = entry.clone();
        } else {
            if next.conversations.len() >= MAX_CONVERSATIONS {
                next.conversations.sort_by_key(|item| item.updated_at_ms);
                next.conversations.remove(0);
            }
            next.conversations.push(entry.clone());
        }
        self.save_locked(&next)?;
        *guard = next;
        Ok(entry)
    }

    /// Removes a conversation from the metadata index. The underlying Claude
    /// session transcript JSONL is left untouched (it is Claude Code's own
    /// source of truth); only the panel's list entry is removed.
    pub fn delete(&self, session_id: &str) -> ApiResult<()> {
        validate_identifier(session_id, "sessionId")?;
        let _lock = self.acquire_lock()?;
        let mut guard = self.value.lock().expect("conversation index poisoned");
        let mut next = self.reload_locked()?;
        let before = next.conversations.len();
        next.conversations
            .retain(|item| item.session_id != session_id);
        if next.conversations.len() == before {
            return Err(ApiError::new(
                "CONVERSATION_NOT_FOUND",
                "找不到指定会话。",
                false,
            ));
        }
        self.save_locked(&next)?;
        *guard = next;
        Ok(())
    }

    pub fn set_status(
        &self,
        session_id: &str,
        run_id: &str,
        status: ConversationStatus,
        now_ms: u64,
    ) -> ApiResult<()> {
        let _lock = self.acquire_lock()?;
        let mut guard = self.value.lock().expect("conversation index poisoned");
        let mut next = self.reload_locked()?;
        let entry = next
            .conversations
            .iter_mut()
            .find(|item| item.session_id == session_id)
            .ok_or_else(|| ApiError::new("CONVERSATION_NOT_FOUND", "找不到指定会话。", false))?;
        if entry.active_run_id.as_deref() != Some(run_id) {
            return Ok(());
        }
        if !status_transition_allowed(&entry.status, &status) {
            return Ok(());
        }
        entry.status = status;
        entry.updated_at_ms = now_ms;
        self.save_locked(&next)?;
        *guard = next;
        Ok(())
    }

    fn acquire_lock(&self) -> ApiResult<std::fs::File> {
        let lock_path = self.path.with_extension("json.lock");
        ensure_regular_parent(&lock_path)?;
        if lock_path.exists() {
            let metadata = fs::symlink_metadata(&lock_path)
                .map_err(|_| ApiError::io("inspect-conversation-lock"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ApiError::new(
                    "CONVERSATION_INDEX_INVALID",
                    "会话索引锁文件无效。",
                    false,
                ));
            }
        }
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|_| ApiError::io("lock-conversation-index"))?;
        lock_file
            .lock_exclusive()
            .map_err(|_| ApiError::io("lock-conversation-index"))?;
        Ok(lock_file)
    }

    fn reload_locked(&self) -> ApiResult<StoredIndex> {
        if !self.path.exists() {
            return Ok(StoredIndex::default());
        }
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| ApiError::io("read-conversation-index"))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > CONVERSATION_FILE_LIMIT
        {
            return Err(ApiError::new(
                "CONVERSATION_INDEX_INVALID",
                "会话索引无效或异常过大。",
                false,
            ));
        }
        let bytes = fs::read(&self.path).map_err(|_| ApiError::io("read-conversation-index"))?;
        let parsed: StoredIndex = serde_json::from_slice(&bytes).map_err(|_| {
            ApiError::new("CONVERSATION_INDEX_INVALID", "会话索引 JSON 无效。", false)
        })?;
        if parsed.schema_version != 1 || parsed.conversations.len() > MAX_CONVERSATIONS {
            return Err(ApiError::new(
                "CONVERSATION_INDEX_UNSUPPORTED",
                "会话索引版本或数量不受支持。",
                false,
            ));
        }
        Ok(parsed)
    }

    fn save_locked(&self, value: &StoredIndex) -> ApiResult<()> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|_| ApiError::io("serialize-conversation-index"))?;
        if bytes.len() > CONVERSATION_FILE_LIMIT as usize {
            return Err(ApiError::new(
                "CONVERSATION_INDEX_TOO_LARGE",
                "会话索引异常过大。",
                false,
            ));
        }
        replace_file_atomically(&self.path, &bytes)
    }

    pub fn rename(&self, session_id: &str, title: &str) -> ApiResult<()> {
        validate_identifier(session_id, "sessionId")?;
        validate_text(title, 256, "title")?;
        self.mutate_entry(session_id, |entry| {
            entry.title = title.trim().to_owned();
        })
    }

    pub fn set_favorite(&self, session_id: &str, favorite: bool) -> ApiResult<()> {
        validate_identifier(session_id, "sessionId")?;
        self.mutate_entry(session_id, |entry| entry.favorite = favorite)
    }

    pub fn set_archived(&self, session_id: &str, archived: bool) -> ApiResult<()> {
        validate_identifier(session_id, "sessionId")?;
        self.mutate_entry(session_id, |entry| entry.archived = archived)
    }

    fn mutate_entry<F>(&self, session_id: &str, mutation: F) -> ApiResult<()>
    where
        F: FnOnce(&mut ConversationMetadata),
    {
        let _lock = self.acquire_lock()?;
        let mut guard = self.value.lock().expect("conversation index poisoned");
        let mut next = self.reload_locked()?;
        let entry = next
            .conversations
            .iter_mut()
            .find(|item| item.session_id == session_id)
            .ok_or_else(|| ApiError::new("CONVERSATION_NOT_FOUND", "找不到指定会话。", false))?;
        mutation(entry);
        entry.updated_at_ms = current_time_ms();
        self.save_locked(&next)?;
        *guard = next;
        Ok(())
    }
    pub fn revision(&self) -> String {
        let guard = self.value.lock().expect("conversation index poisoned");
        let bytes = serde_json::to_vec(&*guard).unwrap_or_default();
        let mut digest = Sha256::new();
        digest.update(bytes);
        hex::encode(digest.finalize())
    }
}

fn status_transition_allowed(current: &ConversationStatus, next: &ConversationStatus) -> bool {
    match current {
        ConversationStatus::Failed => matches!(next, ConversationStatus::Failed),
        ConversationStatus::Completed => {
            matches!(
                next,
                ConversationStatus::Completed | ConversationStatus::Failed
            )
        }
        _ => true,
    }
}

fn ensure_regular_parent(path: &Path) -> ApiResult<()> {
    let parent = path.parent().ok_or_else(|| {
        ApiError::new(
            "CONVERSATION_INDEX_INVALID",
            "会话索引没有有效父目录。",
            false,
        )
    })?;
    if parent.exists() {
        let metadata = fs::symlink_metadata(parent)
            .map_err(|_| ApiError::io("inspect-conversation-directory"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ApiError::new(
                "CONVERSATION_INDEX_INVALID",
                "会话索引目录不是安全的普通目录。",
                false,
            ));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &'static str) -> ApiResult<()> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(ApiError::new("INVALID_CONVERSATION_FIELD", "会话标识无效。", false).field(field))
    }
}

fn validate_text(value: &str, limit: usize, field: &'static str) -> ApiResult<()> {
    if value.is_empty()
        || value.len() > limit
        || value.chars().any(|character| character.is_control())
    {
        return Err(
            ApiError::new("INVALID_CONVERSATION_FIELD", "会话元数据无效。", false).field(field),
        );
    }
    Ok(())
}

fn current_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_metadata_without_message_content() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let index = ConversationIndex::load(path.clone()).unwrap();
        index
            .upsert(UpsertConversation {
                session_id: "3f179a40-6f39-4e8d-a990-b3bfd4669f5a".into(),
                active_run_id: Some("run-one".into()),
                title: "Untitled".into(),
                project_path: "C:\\workspace".into(),
                profile_id: None,
                provider_name: None,
                model_id: None,
                parent_session_id: None,
                now_ms: 1,
                status: ConversationStatus::Idle,
            })
            .unwrap();
        let raw = fs::read_to_string(path).unwrap();
        assert!(!raw.contains("prompt"));
        assert!(!raw.contains("attachment"));
        assert_eq!(index.list().len(), 1);
    }

    #[test]
    fn delete_removes_only_the_matching_session_and_persists() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let index = ConversationIndex::load(path.clone()).unwrap();
        index.upsert(test_input("session-one", "One", 1)).unwrap();
        index.upsert(test_input("session-two", "Two", 2)).unwrap();

        index.delete("session-one").unwrap();
        let ids = index
            .list()
            .into_iter()
            .map(|item| item.session_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["session-two"]);

        let error = index.delete("session-one").unwrap_err();
        assert_eq!(error.code, "CONVERSATION_NOT_FOUND");
        assert_eq!(ConversationIndex::load(path).unwrap().list().len(), 1);
    }

    #[test]
    fn merges_writes_from_independently_loaded_process_views() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let first = ConversationIndex::load(path.clone()).unwrap();
        let stale_second = ConversationIndex::load(path.clone()).unwrap();

        first.upsert(test_input("session-one", "One", 1)).unwrap();
        stale_second
            .upsert(test_input("session-two", "Two", 2))
            .unwrap();

        let reloaded = ConversationIndex::load(path).unwrap();
        let ids = reloaded
            .list()
            .into_iter()
            .map(|item| item.session_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["session-two", "session-one"]);
    }

    #[test]
    fn status_update_reloads_the_latest_index_under_lock() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let first = ConversationIndex::load(path.clone()).unwrap();
        let stale_second = ConversationIndex::load(path.clone()).unwrap();
        first.upsert(test_input("session-one", "One", 1)).unwrap();

        stale_second
            .set_status(
                "session-one",
                "run-session-one",
                ConversationStatus::Running,
                2,
            )
            .unwrap();

        let entry = ConversationIndex::load(path).unwrap().list().remove(0);
        assert_eq!(entry.status, ConversationStatus::Running);
        assert_eq!(entry.updated_at_ms, 2);
    }

    #[test]
    fn terminal_status_cannot_be_reopened_or_downgraded_by_late_event() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let index = ConversationIndex::load(path.clone()).unwrap();
        index.upsert(test_input("session-one", "One", 10)).unwrap();
        index
            .set_status(
                "session-one",
                "run-session-one",
                ConversationStatus::Completed,
                20,
            )
            .unwrap();
        index
            .set_status(
                "session-one",
                "run-session-one",
                ConversationStatus::Running,
                21,
            )
            .unwrap();
        index
            .set_status(
                "session-one",
                "run-session-one",
                ConversationStatus::Failed,
                22,
            )
            .unwrap();
        index
            .set_status(
                "session-one",
                "run-session-one",
                ConversationStatus::Completed,
                23,
            )
            .unwrap();

        let entry = ConversationIndex::load(path).unwrap().list().remove(0);
        assert_eq!(entry.status, ConversationStatus::Failed);
        assert_eq!(entry.updated_at_ms, 22);
    }

    #[test]
    fn stale_run_cannot_overwrite_newer_status() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("conversations.json");
        let index = ConversationIndex::load(path.clone()).unwrap();
        index.upsert(test_input("session-one", "One", 10)).unwrap();

        index
            .set_status("session-one", "run-old", ConversationStatus::Failed, 20)
            .unwrap();

        let entry = ConversationIndex::load(path).unwrap().list().remove(0);
        assert_eq!(entry.status, ConversationStatus::Idle);
        assert_eq!(entry.updated_at_ms, 10);
    }

    fn test_input(session_id: &str, title: &str, now_ms: u64) -> UpsertConversation {
        UpsertConversation {
            session_id: session_id.into(),
            active_run_id: Some(format!("run-{session_id}")),
            title: title.into(),
            project_path: "C:\\workspace".into(),
            profile_id: None,
            provider_name: None,
            model_id: None,
            parent_session_id: None,
            now_ms,
            status: ConversationStatus::Idle,
        }
    }
}
