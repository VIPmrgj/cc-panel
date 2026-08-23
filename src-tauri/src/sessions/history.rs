use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_MAX_FILES_SCANNED: usize = 256;
const DEFAULT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_LINES: usize = 100_000;
const MAX_SESSION_ID_BYTES: usize = 256;

#[derive(Debug, Clone)]
pub struct HistoryLimits {
    pub max_project_dirs: usize,
    pub max_file_bytes: u64,
    pub max_lines: usize,
}

impl Default for HistoryLimits {
    fn default() -> Self {
        Self {
            max_project_dirs: DEFAULT_MAX_FILES_SCANNED,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_lines: DEFAULT_MAX_LINES,
        }
    }
}

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("session id is invalid")]
    InvalidSessionId,
    #[error("the Claude projects directory is unavailable")]
    ProjectsDirectoryUnavailable(#[source] io::Error),
    #[error("the session transcript is ambiguous")]
    Ambiguous,
    #[error("the session transcript was not found")]
    NotFound,
    #[error("the session transcript is not a regular contained file")]
    UnsafePath,
    #[error("the session transcript is too large")]
    TooLarge,
    #[error("the session transcript has too many lines")]
    TooManyLines,
    #[error("the session transcript is not valid UTF-8")]
    InvalidUtf8,
    #[error("the session transcript contains invalid JSON")]
    InvalidJson,
    #[error("the session transcript does not belong to the requested session")]
    SessionMismatch,
    #[error("the session transcript cwd does not match the requested cwd")]
    CwdMismatch,
    #[error("failed to read the session transcript")]
    Read(#[source] io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistorySnapshot {
    pub session_id: String,
    pub cwd: Option<String>,
    pub path: PathBuf,
    pub turns: Vec<HistoryTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryTurn {
    pub role: String,
    pub message_id: Option<String>,
    pub content: Value,
    pub source_records: usize,
}

#[derive(Debug, Clone)]
pub struct HistoryLoader {
    projects_dir: PathBuf,
    limits: HistoryLimits,
}

impl HistoryLoader {
    /// `projects_dir` should be the canonical `~/.claude/projects` directory.
    /// The loader scans only its immediate child directories and never treats
    /// persisted JSONL as live stdin protocol messages.
    pub fn new(projects_dir: impl Into<PathBuf>) -> Self {
        Self {
            projects_dir: projects_dir.into(),
            limits: HistoryLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: HistoryLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn projects_dir(&self) -> &Path {
        &self.projects_dir
    }

    pub fn load(
        &self,
        session_id: &str,
        expected_cwd: Option<&Path>,
    ) -> Result<HistorySnapshot, HistoryError> {
        validate_session_id(session_id)?;
        let root = fs::canonicalize(&self.projects_dir)
            .map_err(HistoryError::ProjectsDirectoryUnavailable)?;
        let entries = fs::read_dir(&root).map_err(HistoryError::ProjectsDirectoryUnavailable)?;
        let mut candidates = Vec::new();
        for (index, entry) in entries.enumerate() {
            if index >= self.limits.max_project_dirs {
                return Err(HistoryError::TooLarge);
            }
            let entry = entry.map_err(HistoryError::ProjectsDirectoryUnavailable)?;
            let metadata = entry
                .path()
                .symlink_metadata()
                .map_err(HistoryError::ProjectsDirectoryUnavailable)?;
            if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                continue;
            }
            if !metadata.is_dir() {
                continue;
            }
            let path = entry.path().join(format!("{session_id}.jsonl"));
            let file = match self.open_history_file(&path) {
                Ok(file) => file,
                Err(HistoryError::Read(error)) if error.kind() == io::ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let parent = fs::canonicalize(entry.path()).map_err(HistoryError::Read)?;
            if !is_contained(&root, &parent) || path.parent() != Some(entry.path().as_path()) {
                return Err(HistoryError::UnsafePath);
            }
            candidates.push((path, file));
            if candidates.len() > 1 {
                return Err(HistoryError::Ambiguous);
            }
        }
        let (path, file) = candidates
            .into_iter()
            .next()
            .ok_or(HistoryError::NotFound)?;
        self.parse_file(session_id, expected_cwd, path, file)
    }

    fn open_history_file(&self, path: &Path) -> Result<File, HistoryError> {
        let mut options = OpenOptions::new();
        options.read(true);
        configure_no_follow(&mut options);
        let file = options.open(path).map_err(HistoryError::Read)?;
        let metadata = file.metadata().map_err(HistoryError::Read)?;
        if !metadata.is_file() {
            return Err(HistoryError::UnsafePath);
        }
        if metadata.len() > self.limits.max_file_bytes {
            return Err(HistoryError::TooLarge);
        }
        Ok(file)
    }

    fn parse_file(
        &self,
        session_id: &str,
        expected_cwd: Option<&Path>,
        path: PathBuf,
        file: File,
    ) -> Result<HistorySnapshot, HistoryError> {
        let mut bytes = Vec::new();
        file.take(self.limits.max_file_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(HistoryError::Read)?;
        if bytes.len() as u64 > self.limits.max_file_bytes {
            return Err(HistoryError::TooLarge);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| HistoryError::InvalidUtf8)?;
        let mut turns: Vec<HistoryTurn> = Vec::new();
        let mut assistant_indexes = HashMap::<String, usize>::new();
        let mut transcript_session = None;
        let mut transcript_cwd = None;
        let mut lines = 0;
        for line in text.lines() {
            lines += 1;
            if lines > self.limits.max_lines {
                return Err(HistoryError::TooManyLines);
            }
            if line.trim().is_empty() {
                continue;
            }
            let object: Value =
                serde_json::from_str(line).map_err(|_| HistoryError::InvalidJson)?;
            let Some(object_map) = object.as_object() else {
                continue;
            };
            if let Some(id) = string_field(object_map, &["sessionId", "session_id"]) {
                if id != session_id {
                    return Err(HistoryError::SessionMismatch);
                }
                transcript_session = Some(id.to_owned());
            }
            if transcript_cwd.is_none() {
                transcript_cwd =
                    string_field(object_map, &["cwd", "workingDirectory"]).map(str::to_owned);
            }
            if let Some(expected) = expected_cwd {
                if let Some(actual) = transcript_cwd.as_deref() {
                    if !same_path(actual, expected) {
                        return Err(HistoryError::CwdMismatch);
                    }
                }
            }
            let record_type = object_map.get("type").and_then(Value::as_str);
            let message = object_map.get("message").and_then(Value::as_object);
            let Some(message) = message else { continue };
            let role = message.get("role").and_then(Value::as_str);
            let Some(role) = role else { continue };
            if role != "user" && role != "assistant" {
                continue;
            }
            let content = message.get("content").cloned().unwrap_or(Value::Null);
            // Metadata, attachment, file-history, and queue records are not
            // conversation turns even if a future version happens to include a
            // nested message-like object.
            if matches!(
                record_type,
                Some(
                    "file-history-snapshot"
                        | "attachment"
                        | "queue-operation"
                        | "progress"
                        | "metadata"
                )
            ) {
                continue;
            }
            let message_id = message
                .get("id")
                .or_else(|| message.get("message_id"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            if role == "assistant" {
                if let Some(id) = message_id.as_deref() {
                    if let Some(index) = assistant_indexes.get(id).copied() {
                        merge_content(&mut turns[index].content, content);
                        turns[index].source_records += 1;
                        continue;
                    }
                    assistant_indexes.insert(id.to_owned(), turns.len());
                }
            }
            turns.push(HistoryTurn {
                role: role.to_owned(),
                message_id,
                content,
                source_records: 1,
            });
        }
        let session_id = transcript_session.ok_or(HistoryError::SessionMismatch)?;
        Ok(HistorySnapshot {
            session_id,
            cwd: transcript_cwd,
            path,
            turns,
        })
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn configure_no_follow(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(unix)]
fn configure_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(any(unix, windows)))]
fn configure_no_follow(_: &mut OpenOptions) {}

fn merge_content(existing: &mut Value, incoming: Value) {
    match (existing, incoming) {
        (Value::Array(existing), Value::Array(incoming)) => existing.extend(incoming),
        (Value::Array(existing), incoming) => existing.push(incoming),
        (existing @ Value::Null, incoming) => *existing = incoming,
        (existing, incoming) => {
            *existing = Value::Array(vec![existing.take(), incoming]);
        }
    }
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    fields: &[&str],
) -> Option<&'a str> {
    fields
        .iter()
        .find_map(|field| object.get(*field).and_then(Value::as_str))
}

fn same_path(actual: &str, expected: &Path) -> bool {
    fs::canonicalize(actual)
        .ok()
        .zip(fs::canonicalize(expected).ok())
        .is_some_and(|(actual, expected)| actual == expected)
}

fn is_contained(root: &Path, child: &Path) -> bool {
    child == root || child.starts_with(root)
}

fn validate_session_id(session_id: &str) -> Result<(), HistoryError> {
    if session_id.is_empty()
        || session_id.len() > MAX_SESSION_ID_BYTES
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(HistoryError::InvalidSessionId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_transcript(root: &Path, dir: &str, session: &str, lines: &[Value]) -> PathBuf {
        let project = root.join(dir);
        fs::create_dir_all(&project).unwrap();
        let path = project.join(format!("{session}.jsonl"));
        let body = lines
            .iter()
            .map(|line| serde_json::to_string(line).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{body}\n")).unwrap();
        path
    }

    #[test]
    fn scans_immediate_project_dirs_and_groups_assistant_fragments() {
        let temp = tempdir().unwrap();
        let cwd = temp.path().join("project");
        fs::create_dir_all(&cwd).unwrap();
        let session = "abc-123";
        let lines = vec![
            serde_json::json!({"type":"system","sessionId":session,"cwd":cwd}),
            serde_json::json!({"type":"user","sessionId":session,"message":{"role":"user","content":"hi"}}),
            serde_json::json!({"type":"assistant","sessionId":session,"message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"a"}]}}),
            serde_json::json!({"type":"assistant","sessionId":session,"message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"b"}]}}),
        ];
        let path = write_transcript(temp.path(), "encoded", session, &lines);
        let snapshot = HistoryLoader::new(temp.path())
            .load(session, Some(&cwd))
            .unwrap();
        assert_eq!(snapshot.path, fs::canonicalize(path).unwrap());
        assert_eq!(snapshot.turns.len(), 2);
        assert_eq!(snapshot.turns[1].source_records, 2);
        assert_eq!(snapshot.turns[1].content.as_array().unwrap().len(), 2);
    }

    #[test]
    fn ignores_metadata_attachment_and_file_history_records() {
        let temp = tempdir().unwrap();
        let session = "abc";
        let lines = vec![
            serde_json::json!({"type":"system","sessionId":session}),
            serde_json::json!({"type":"attachment","message":{"role":"user","content":"ignore"}}),
            serde_json::json!({"type":"file-history-snapshot","message":{"role":"assistant","content":"ignore"}}),
            serde_json::json!({"type":"user","sessionId":session,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":"ok"}]}}),
        ];
        write_transcript(temp.path(), "encoded", session, &lines);
        let snapshot = HistoryLoader::new(temp.path()).load(session, None).unwrap();
        assert_eq!(snapshot.turns.len(), 1);
        assert_eq!(snapshot.turns[0].role, "user");
    }

    #[test]
    fn rejects_ambiguous_and_mismatched_transcripts() {
        let temp = tempdir().unwrap();
        let session = "abc";
        let line = serde_json::json!({"type":"system","sessionId":session});
        write_transcript(temp.path(), "one", session, std::slice::from_ref(&line));
        write_transcript(temp.path(), "two", session, std::slice::from_ref(&line));
        assert!(matches!(
            HistoryLoader::new(temp.path()).load(session, None),
            Err(HistoryError::Ambiguous)
        ));
    }

    #[test]
    fn ignores_symlinked_project_directories() {
        let temp = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let session = "abc";
        write_transcript(
            outside.path(),
            "real-project",
            session,
            &[serde_json::json!({"type":"system","sessionId":session})],
        );
        let real_project = outside.path().join("real-project");
        let linked_project = temp.path().join("linked-project");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_project, &linked_project).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real_project, &linked_project).unwrap();

        assert!(matches!(
            HistoryLoader::new(temp.path()).load(session, None),
            Err(HistoryError::NotFound)
        ));
    }

    #[test]
    fn never_recurses_into_nested_directories() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("outer").join("nested");
        let session = "abc";
        write_transcript(
            &nested,
            "encoded",
            session,
            &[serde_json::json!({"sessionId":session})],
        );
        assert!(matches!(
            HistoryLoader::new(temp.path()).load(session, None),
            Err(HistoryError::NotFound)
        ));
    }
}
