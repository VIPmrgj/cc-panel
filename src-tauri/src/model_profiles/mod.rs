//! Persistent custom model profiles with OS-protected API keys.
//!
//! The caller supplies the target path (normally `~/.cc-panel/models.json`).
//! Profile views are serializable for IPC, but secret-bearing types are not.

mod connection;
mod native_prompt;
pub use connection::test_connection;
mod protection;

pub(crate) use native_prompt::prompt_api_key;

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};
use url::{Host, Url};
use uuid::Uuid;

use crate::{
    dto::{ApiError, ApiResult},
    platform::replace_file_atomically,
};

pub const MODEL_PROFILES_SCHEMA_VERSION: u32 = 1;
pub const MODEL_PROFILES_FILE_LIMIT: u64 = 256 * 1024;

const PROVIDER_NAME_LIMIT: usize = 120;
const NOTE_LIMIT: usize = 2_000;
const MODEL_ID_LIMIT: usize = 512;
const API_KEY_LIMIT: usize = 16 * 1024;
const PROTECTED_VALUE_LIMIT: usize = 64 * 1024;
const WINDOWS_DPAPI_SCHEME: &str = "windows-dpapi-current-user-v1";

/// Renderer-safe input accepted by model profile commands.
///
/// The strict deserializer rejects secret-bearing or otherwise unknown fields,
/// so even a hand-written webview invocation cannot send an API key through
/// ordinary IPC. New profiles must use the native credential prompt command.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelProfileInput {
    #[serde(default)]
    pub id: Option<String>,
    pub provider_name: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    pub base_url: String,
    pub model_id: String,
    #[serde(default)]
    pub selected: bool,
}

/// A safe-to-serialize profile view. It never contains protected bytes or a
/// plaintext API key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileView {
    pub id: String,
    pub provider_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    pub base_url: String,
    pub model_id: String,
    pub selected: bool,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfilesView {
    pub schema_version: u32,
    pub revision: u64,
    pub profiles: Vec<ModelProfileView>,
}

/// Secret resolution result for constructing a provider client. This type is
/// deliberately not serializable and redacts its `Debug` output.
pub struct ResolvedModelSecret {
    pub profile: ModelProfileView,
    api_key: SecretString,
}

impl ResolvedModelSecret {
    pub fn api_key(&self) -> &str {
        self.api_key.expose()
    }
}

impl std::fmt::Debug for ResolvedModelSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedModelSecret")
            .field("profile", &self.profile)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

struct SecretString(String);

impl SecretString {
    fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // SAFETY: only byte values are overwritten, the length and capacity do
        // not change, and the string is never observed again after `drop`.
        unsafe {
            self.0.as_mut_vec().fill(0);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredDocument {
    schema_version: u32,
    revision: u64,
    profiles: Vec<StoredProfile>,
}

impl Default for StoredDocument {
    fn default() -> Self {
        Self {
            schema_version: MODEL_PROFILES_SCHEMA_VERSION,
            revision: 0,
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    id: String,
    provider_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    website_url: Option<String>,
    base_url: String,
    model_id: String,
    selected: bool,
    protected_api_key: ProtectedValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtectedValue {
    scheme: String,
    ciphertext: String,
}

pub struct ModelProfileStore {
    target: PathBuf,
    access: Mutex<()>,
}

impl ModelProfileStore {
    /// Opens a store at the caller-supplied target without creating the file.
    /// Existing content is fully parsed and validated before construction
    /// succeeds.
    pub fn new(target: impl Into<PathBuf>) -> ApiResult<Self> {
        let store = Self {
            target: target.into(),
            access: Mutex::new(()),
        };
        store.read_document()?;
        Ok(store)
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn list(&self) -> ApiResult<ModelProfilesView> {
        let _guard = self.lock()?;
        Ok(view_for(self.read_document()?))
    }

    /// Updates an existing profile while preserving its protected key.
    pub fn update(
        &self,
        input: ModelProfileInput,
        expected_revision: u64,
    ) -> ApiResult<ModelProfilesView> {
        self.save_internal(input, None, expected_revision, false)
    }

    /// Stores a key obtained from the native credential prompt. Unlike the
    /// renderer-safe update path, this also permits creation.
    pub fn save_with_api_key(
        &self,
        input: ModelProfileInput,
        api_key: String,
        expected_revision: u64,
    ) -> ApiResult<ModelProfilesView> {
        self.save_internal(input, Some(api_key), expected_revision, true)
    }

    fn save_internal(
        &self,
        mut input: ModelProfileInput,
        api_key: Option<String>,
        expected_revision: u64,
        allow_create: bool,
    ) -> ApiResult<ModelProfilesView> {
        let _guard = self.lock()?;
        let mut document = self.read_document()?;
        ensure_revision(&document, expected_revision)?;

        let requested_id = input.id.take();
        if let Some(id) = requested_id.as_deref() {
            validate_id(id)?;
        } else if !allow_create {
            return Err(invalid_field("id", "新模型配置必须通过系统凭据窗口保存。"));
        }
        let existing_index = requested_id.as_deref().and_then(|id| {
            document
                .profiles
                .iter()
                .position(|profile| profile.id == id)
        });
        if requested_id.is_some() && existing_index.is_none() {
            return Err(not_found(requested_id.as_deref().unwrap_or_default()));
        }

        let provider_name = validate_label(
            "providerName",
            input.provider_name,
            PROVIDER_NAME_LIMIT,
            false,
        )?;
        let note = input
            .note
            .map(|note| validate_label("note", note, NOTE_LIMIT, false))
            .transpose()?;
        let website_url = input
            .website_url
            .map(|url| validate_service_url("websiteUrl", &url))
            .transpose()?;
        let base_url = validate_service_url("baseUrl", &input.base_url)?;
        let model_id = validate_model_id(input.model_id)?;
        let protected_api_key = match api_key {
            Some(api_key) => protect_api_key(api_key)?,
            None => existing_index
                .map(|index| document.profiles[index].protected_api_key.clone())
                .ok_or_else(missing_api_key)?,
        };
        let id = requested_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let profile = StoredProfile {
            id,
            provider_name,
            note,
            website_url,
            base_url,
            model_id,
            selected: input.selected,
            protected_api_key,
        };

        if input.selected {
            for current in &mut document.profiles {
                current.selected = false;
            }
        }
        match existing_index {
            Some(index) => document.profiles[index] = profile,
            None => document.profiles.push(profile),
        }
        self.persist_next(&mut document)?;
        Ok(view_for(document))
    }

    pub fn delete(&self, id: &str, expected_revision: u64) -> ApiResult<ModelProfilesView> {
        validate_id(id)?;
        let _guard = self.lock()?;
        let mut document = self.read_document()?;
        ensure_revision(&document, expected_revision)?;
        let index = document
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| not_found(id))?;
        document.profiles.remove(index);
        self.persist_next(&mut document)?;
        Ok(view_for(document))
    }

    pub fn select(&self, id: &str, expected_revision: u64) -> ApiResult<ModelProfilesView> {
        validate_id(id)?;
        let _guard = self.lock()?;
        let mut document = self.read_document()?;
        ensure_revision(&document, expected_revision)?;
        if !document.profiles.iter().any(|profile| profile.id == id) {
            return Err(not_found(id));
        }
        for profile in &mut document.profiles {
            profile.selected = profile.id == id;
        }
        self.persist_next(&mut document)?;
        Ok(view_for(document))
    }

    pub fn clear_selection(&self, expected_revision: u64) -> ApiResult<ModelProfilesView> {
        let _guard = self.lock()?;
        let mut document = self.read_document()?;
        ensure_revision(&document, expected_revision)?;
        for profile in &mut document.profiles {
            profile.selected = false;
        }
        self.persist_next(&mut document)?;
        Ok(view_for(document))
    }

    /// Decrypts a profile API key only for an explicit internal call. The
    /// return value has no `Serialize` implementation and redacts `Debug`.
    pub fn resolve_secret(&self, id: &str) -> ApiResult<ResolvedModelSecret> {
        validate_id(id)?;
        let _guard = self.lock()?;
        let document = self.read_document()?;
        let profile = document
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| not_found(id))?;
        let api_key = unprotect_api_key(&profile.protected_api_key)?;
        Ok(ResolvedModelSecret {
            profile: profile_view(profile),
            api_key,
        })
    }

    fn lock(&self) -> ApiResult<MutexGuard<'_, ()>> {
        self.access.lock().map_err(|_| {
            ApiError::new(
                "MODEL_PROFILES_STORE_UNAVAILABLE",
                "模型配置存储暂时不可用。",
                true,
            )
        })
    }

    fn read_document(&self) -> ApiResult<StoredDocument> {
        validate_target(&self.target)?;
        if !self.target.exists() {
            return Ok(StoredDocument::default());
        }

        let metadata =
            fs::metadata(&self.target).map_err(|_| ApiError::io("read-model-profiles-metadata"))?;
        if metadata.len() > MODEL_PROFILES_FILE_LIMIT {
            return Err(file_too_large());
        }
        let file = File::open(&self.target).map_err(|_| ApiError::io("open-model-profiles"))?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(MODEL_PROFILES_FILE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ApiError::io("read-model-profiles"))?;
        if bytes.len() as u64 > MODEL_PROFILES_FILE_LIMIT {
            return Err(file_too_large());
        }
        let document: StoredDocument = serde_json::from_slice(&bytes).map_err(|_| {
            ApiError::new(
                "INVALID_MODEL_PROFILES",
                "models.json 不是有效的模型配置文件。",
                false,
            )
        })?;
        validate_document(&document)?;
        Ok(document)
    }

    fn persist_next(&self, document: &mut StoredDocument) -> ApiResult<()> {
        document.revision = document.revision.checked_add(1).ok_or_else(|| {
            ApiError::new(
                "MODEL_PROFILES_REVISION_EXHAUSTED",
                "模型配置版本号已耗尽，无法继续保存。",
                false,
            )
        })?;
        let mut bytes = serde_json::to_vec_pretty(document).map_err(|_| {
            ApiError::new(
                "MODEL_PROFILES_SERIALIZATION_FAILED",
                "无法序列化模型配置。",
                false,
            )
        })?;
        bytes.push(b'\n');
        if bytes.len() > MODEL_PROFILES_FILE_LIMIT as usize {
            return Err(file_too_large());
        }
        replace_file_atomically(&self.target, &bytes)
    }
}

fn validate_document(document: &StoredDocument) -> ApiResult<()> {
    if document.schema_version != MODEL_PROFILES_SCHEMA_VERSION {
        return Err(ApiError::new(
            "UNSUPPORTED_MODEL_PROFILES_SCHEMA",
            "models.json 的格式版本不受支持。",
            false,
        ));
    }

    let mut selected_count = 0usize;
    for (index, profile) in document.profiles.iter().enumerate() {
        validate_id(&profile.id).map_err(|_| invalid_stored_document())?;
        if document.profiles[..index]
            .iter()
            .any(|previous| previous.id == profile.id)
        {
            return Err(invalid_stored_document());
        }
        validate_label(
            "providerName",
            profile.provider_name.clone(),
            PROVIDER_NAME_LIMIT,
            false,
        )
        .map_err(|_| invalid_stored_document())?;
        if let Some(note) = &profile.note {
            validate_label("note", note.clone(), NOTE_LIMIT, false)
                .map_err(|_| invalid_stored_document())?;
        }
        if let Some(website_url) = &profile.website_url {
            validate_service_url("websiteUrl", website_url)
                .map_err(|_| invalid_stored_document())?;
        }
        validate_service_url("baseUrl", &profile.base_url)
            .map_err(|_| invalid_stored_document())?;
        validate_model_id(profile.model_id.clone()).map_err(|_| invalid_stored_document())?;
        validate_protected_value(&profile.protected_api_key)
            .map_err(|_| invalid_stored_document())?;
        selected_count += usize::from(profile.selected);
    }
    if selected_count > 1 {
        return Err(invalid_stored_document());
    }
    Ok(())
}

fn validate_target(target: &Path) -> ApiResult<()> {
    let parent = target.parent().ok_or_else(|| {
        ApiError::new(
            "UNSAFE_MODEL_PROFILES_PATH",
            "models.json 路径没有有效父目录。",
            false,
        )
    })?;
    if target.exists() {
        let metadata =
            fs::symlink_metadata(target).map_err(|_| ApiError::io("inspect-model-profiles"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ApiError::new(
                "UNSAFE_MODEL_PROFILES_PATH",
                "拒绝读取或写入符号链接及非普通 models.json。",
                false,
            ));
        }
    }
    if parent.exists() {
        let metadata = fs::symlink_metadata(parent)
            .map_err(|_| ApiError::io("inspect-model-profiles-directory"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ApiError::new(
                "UNSAFE_MODEL_PROFILES_PATH",
                "models.json 的父路径不是安全的普通目录。",
                false,
            ));
        }
    }
    Ok(())
}

fn ensure_revision(document: &StoredDocument, expected: u64) -> ApiResult<()> {
    if document.revision != expected {
        return Err(ApiError::new(
            "MODEL_PROFILES_CONFLICT",
            "模型配置已被其他操作修改，请刷新后重试。",
            true,
        ));
    }
    Ok(())
}

fn validate_id(id: &str) -> ApiResult<()> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id.trim() == id
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !valid {
        return Err(invalid_field("id", "模型配置 ID 无效。"));
    }
    Ok(())
}

fn validate_label(
    field: &'static str,
    value: String,
    limit: usize,
    allow_empty: bool,
) -> ApiResult<String> {
    let valid = value.len() <= limit
        && value.trim() == value
        && (allow_empty || !value.is_empty())
        && !value.chars().any(char::is_control);
    if !valid {
        return Err(invalid_field(field, "模型配置文本字段无效。"));
    }
    Ok(value)
}

fn validate_model_id(model_id: String) -> ApiResult<String> {
    let valid = !model_id.is_empty()
        && model_id.len() <= MODEL_ID_LIMIT
        && model_id.trim() == model_id
        && !model_id
            .chars()
            .any(|character| character.is_control() || character.is_whitespace());
    if !valid {
        return Err(invalid_field("modelId", "模型 ID 无效。"));
    }
    Ok(model_id)
}

fn validate_service_url(field: &'static str, value: &str) -> ApiResult<String> {
    if value.is_empty()
        || value.len() > 2_048
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(invalid_url(field));
    }
    let url = Url::parse(value).map_err(|_| invalid_url(field))?;
    let loopback = match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    let secure_transport = url.scheme() == "https" || (url.scheme() == "http" && loopback);
    if !secure_transport
        || url.cannot_be_a_base()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host().is_none()
    {
        return Err(invalid_url(field));
    }
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn protect_api_key(api_key: String) -> ApiResult<ProtectedValue> {
    if api_key.is_empty()
        || api_key.len() > API_KEY_LIMIT
        || api_key.trim() != api_key
        || api_key.chars().any(char::is_control)
    {
        return Err(invalid_field("apiKey", "API Key 无效。"));
    }
    let mut plaintext = api_key.into_bytes();
    let protected = protection::protect(&mut plaintext);
    plaintext.fill(0);
    let ciphertext = protected?;
    if ciphertext.len() > PROTECTED_VALUE_LIMIT {
        return Err(ApiError::new(
            "MODEL_SECRET_PROTECTION_FAILED",
            "受保护的 API Key 超过安全上限。",
            false,
        ));
    }
    Ok(ProtectedValue {
        scheme: WINDOWS_DPAPI_SCHEME.into(),
        ciphertext: hex::encode(ciphertext),
    })
}

fn unprotect_api_key(value: &ProtectedValue) -> ApiResult<SecretString> {
    validate_protected_value(value)?;
    let mut ciphertext = hex::decode(&value.ciphertext).map_err(|_| invalid_protected_value())?;
    let plaintext = protection::unprotect(&mut ciphertext);
    ciphertext.fill(0);
    let plaintext = plaintext?;
    match String::from_utf8(plaintext) {
        Ok(api_key) if !api_key.is_empty() && api_key.len() <= API_KEY_LIMIT => {
            Ok(SecretString(api_key))
        }
        Ok(mut invalid) => {
            // SAFETY: this value is immediately dropped and is not observed.
            unsafe {
                invalid.as_mut_vec().fill(0);
            }
            Err(invalid_protected_value())
        }
        Err(error) => {
            let mut invalid = error.into_bytes();
            invalid.fill(0);
            Err(invalid_protected_value())
        }
    }
}

fn validate_protected_value(value: &ProtectedValue) -> ApiResult<()> {
    if value.scheme != WINDOWS_DPAPI_SCHEME
        || value.ciphertext.is_empty()
        || value.ciphertext.len() > PROTECTED_VALUE_LIMIT * 2
        || !value.ciphertext.len().is_multiple_of(2)
        || !value
            .ciphertext
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid_protected_value());
    }
    Ok(())
}

fn view_for(document: StoredDocument) -> ModelProfilesView {
    ModelProfilesView {
        schema_version: document.schema_version,
        revision: document.revision,
        profiles: document.profiles.iter().map(profile_view).collect(),
    }
}

fn profile_view(profile: &StoredProfile) -> ModelProfileView {
    ModelProfileView {
        id: profile.id.clone(),
        provider_name: profile.provider_name.clone(),
        note: profile.note.clone(),
        website_url: profile.website_url.clone(),
        base_url: profile.base_url.clone(),
        model_id: profile.model_id.clone(),
        selected: profile.selected,
        has_api_key: true,
    }
}

fn invalid_field(field: &'static str, message: &'static str) -> ApiError {
    ApiError::new("INVALID_MODEL_PROFILE", message, false).field(field)
}

fn invalid_url(field: &'static str) -> ApiError {
    invalid_field(
        field,
        "地址必须使用 HTTPS（本机回环可使用 HTTP），且不能包含凭据、查询或片段。",
    )
}

fn missing_api_key() -> ApiError {
    invalid_field("apiKey", "新模型配置必须提供 API Key。")
}

fn not_found(_id: &str) -> ApiError {
    ApiError::new("MODEL_PROFILE_NOT_FOUND", "找不到指定的模型配置。", false).field("id")
}

fn invalid_protected_value() -> ApiError {
    ApiError::new(
        "MODEL_SECRET_INVALID",
        "模型 API Key 的受保护数据无效或已损坏。",
        false,
    )
}

fn invalid_stored_document() -> ApiError {
    ApiError::new(
        "INVALID_MODEL_PROFILES",
        "models.json 包含无效或冲突的模型配置。",
        false,
    )
}

fn file_too_large() -> ApiError {
    ApiError::new(
        "MODEL_PROFILES_FILE_TOO_LARGE",
        "models.json 超过安全大小上限。",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: Option<&str>, provider: &str, selected: bool) -> ModelProfileInput {
        ModelProfileInput {
            id: id.map(str::to_owned),
            provider_name: provider.into(),
            note: Some("Private endpoint".into()),
            website_url: Some("https://provider.example/docs".into()),
            base_url: "https://provider.example/anthropic".into(),
            model_id: "deepseek-v4-pro[1m]".into(),
            selected,
        }
    }

    fn save(
        store: &ModelProfileStore,
        input: ModelProfileInput,
        revision: u64,
    ) -> ApiResult<ModelProfilesView> {
        store.save_with_api_key(input, "sk-test-super-secret".into(), revision)
    }

    #[test]
    fn empty_store_is_not_created_and_has_schema_revision() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("nested").join("models.json");
        let store = ModelProfileStore::new(target.clone()).unwrap();
        assert!(!target.exists());
        assert_eq!(store.target(), target);
        assert_eq!(
            store.list().unwrap(),
            ModelProfilesView {
                schema_version: MODEL_PROFILES_SCHEMA_VERSION,
                revision: 0,
                profiles: Vec::new(),
            }
        );
    }

    #[cfg(windows)]
    #[test]
    fn stores_only_dpapi_ciphertext_and_resolves_for_current_user() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("models.json");
        let store = ModelProfileStore::new(target.clone()).unwrap();
        let view = save(&store, input(None, "DeepSeek", true), 0).unwrap();
        let id = view.profiles[0].id.clone();
        let bytes = fs::read(&target).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        assert!(!text.contains("sk-test-super-secret"));
        assert!(!text.contains("apiKey\""));
        assert!(text.contains(WINDOWS_DPAPI_SCHEME));
        let resolved = store.resolve_secret(&id).unwrap();
        assert_eq!(resolved.api_key(), "sk-test-super-secret");
        assert!(format!("{resolved:?}").contains("[REDACTED]"));
        assert!(!format!("{resolved:?}").contains("sk-test-super-secret"));
    }

    #[cfg(not(windows))]
    #[test]
    fn refuses_to_store_secret_without_platform_protection() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("models.json");
        let store = ModelProfileStore::new(target.clone()).unwrap();
        let error = save(&store, input(None, "Provider", false), 0).unwrap_err();
        assert_eq!(error.code, "MODEL_SECRET_PROTECTION_UNAVAILABLE");
        assert!(!target.exists());
    }

    #[cfg(windows)]
    #[test]
    fn update_preserves_secret_and_selection_is_exclusive() {
        let temp = tempfile::tempdir().unwrap();
        let store = ModelProfileStore::new(temp.path().join("models.json")).unwrap();
        let first = save(&store, input(None, "First", true), 0).unwrap();
        let first_id = first.profiles[0].id.clone();
        let second = save(&store, input(None, "Second", true), 1).unwrap();
        let second_id = second.profiles[1].id.clone();
        assert!(!second.profiles[0].selected);
        assert!(second.profiles[1].selected);

        let update = input(Some(&first_id), "First renamed", false);
        let updated = store.update(update, 2).unwrap();
        assert_eq!(updated.profiles[0].provider_name, "First renamed");
        assert_eq!(
            store.resolve_secret(&first_id).unwrap().api_key(),
            "sk-test-super-secret"
        );

        let selected = store.select(&first_id, 3).unwrap();
        assert!(selected.profiles[0].selected);
        assert!(!selected.profiles[1].selected);
        let cleared = store.clear_selection(4).unwrap();
        assert!(cleared.profiles.iter().all(|profile| !profile.selected));
        let deleted = store.delete(&second_id, 5).unwrap();
        assert_eq!(deleted.revision, 6);
        assert_eq!(deleted.profiles.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn rejects_stale_revision_without_changing_file() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("models.json");
        let store = ModelProfileStore::new(target.clone()).unwrap();
        save(&store, input(None, "First", false), 0).unwrap();
        let before = fs::read(&target).unwrap();
        let error = save(&store, input(None, "Second", false), 0).unwrap_err();
        assert_eq!(error.code, "MODEL_PROFILES_CONFLICT");
        assert_eq!(fs::read(target).unwrap(), before);
    }

    #[test]
    fn renderer_input_rejects_secret_and_unknown_fields() {
        let payload = serde_json::json!({
            "providerName": "Provider",
            "baseUrl": "https://provider.example",
            "modelId": "model",
            "selected": false,
            "apiKey": "must-not-cross-ipc"
        });
        assert!(serde_json::from_value::<ModelProfileInput>(payload).is_err());

        let payload = serde_json::json!({
            "providerName": "Provider",
            "baseUrl": "https://provider.example",
            "modelId": "model",
            "selected": false,
            "unexpected": true
        });
        assert!(serde_json::from_value::<ModelProfileInput>(payload).is_err());
    }

    #[test]
    fn ordinary_save_cannot_create_a_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ModelProfileStore::new(temp.path().join("models.json")).unwrap();
        let error = store.update(input(None, "Provider", false), 0).unwrap_err();
        assert_eq!(error.code, "INVALID_MODEL_PROFILE");
        assert_eq!(error.field.as_deref(), Some("id"));
        assert!(!store.target().exists());
    }

    #[test]
    fn safe_view_serialization_has_no_secret_fields() {
        let view = ModelProfileView {
            id: "profile-1".into(),
            provider_name: "Provider".into(),
            note: None,
            website_url: None,
            base_url: "https://provider.example".into(),
            model_id: "claude-custom".into(),
            selected: true,
            has_api_key: true,
        };
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json["hasApiKey"], true);
        assert!(json.get("apiKey").is_none());
        assert!(json.get("protectedApiKey").is_none());
        assert!(json.get("ciphertext").is_none());
    }

    #[test]
    fn validates_anthropic_compatible_urls() {
        for valid in [
            "https://api.example.com",
            "https://api.example.com/anthropic/v1",
            "http://localhost:8080/v1",
            "http://127.0.0.1:8080/v1",
            "http://[::1]:8080/v1",
        ] {
            assert!(validate_service_url("baseUrl", valid).is_ok(), "{valid}");
        }
        for invalid in [
            "http://api.example.com",
            "https://user:pass@api.example.com",
            "https://api.example.com/v1?token=x",
            "https://api.example.com/v1#fragment",
            "ftp://api.example.com",
            " https://api.example.com",
            "https://api.example.com/\npath",
        ] {
            let error = validate_service_url("baseUrl", invalid).unwrap_err();
            assert_eq!(error.field.as_deref(), Some("baseUrl"), "{invalid}");
        }
    }

    #[test]
    fn validates_labels_ids_and_model_ids() {
        assert!(validate_label("providerName", "供应商".into(), 120, false).is_ok());
        assert!(validate_label("providerName", " bad".into(), 120, false).is_err());
        assert!(validate_label("providerName", "bad\nlabel".into(), 120, false).is_err());
        assert!(validate_label("note", "line\tnote".into(), 2_000, false).is_err());
        assert!(validate_id("profile_1.test-id").is_ok());
        assert!(validate_id("bad/id").is_err());
        assert!(validate_model_id("deepseek-v4-pro[1m]".into()).is_ok());
        assert!(validate_model_id("model with spaces".into()).is_err());
        assert!(validate_model_id("model\nname".into()).is_err());
    }

    #[test]
    fn rejects_oversized_or_invalid_existing_documents() {
        let temp = tempfile::tempdir().unwrap();
        let oversized = temp.path().join("oversized.json");
        fs::write(
            &oversized,
            vec![b'x'; MODEL_PROFILES_FILE_LIMIT as usize + 1],
        )
        .unwrap();
        assert_eq!(
            ModelProfileStore::new(oversized).err().unwrap().code,
            "MODEL_PROFILES_FILE_TOO_LARGE"
        );

        let wrong_schema = temp.path().join("wrong-schema.json");
        fs::write(
            &wrong_schema,
            br#"{"schemaVersion":99,"revision":0,"profiles":[]}"#,
        )
        .unwrap();
        assert_eq!(
            ModelProfileStore::new(wrong_schema).err().unwrap().code,
            "UNSUPPORTED_MODEL_PROFILES_SCHEMA"
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_multiple_selected_profiles() {
        let protected = ProtectedValue {
            scheme: WINDOWS_DPAPI_SCHEME.into(),
            ciphertext: "00".into(),
        };
        let profile = StoredProfile {
            id: "same".into(),
            provider_name: "Provider".into(),
            note: None,
            website_url: None,
            base_url: "https://provider.example".into(),
            model_id: "model".into(),
            selected: true,
            protected_api_key: protected,
        };
        let document = StoredDocument {
            schema_version: MODEL_PROFILES_SCHEMA_VERSION,
            revision: 1,
            profiles: vec![profile.clone(), profile],
        };
        assert_eq!(
            validate_document(&document).unwrap_err().code,
            "INVALID_MODEL_PROFILES"
        );
    }

    #[cfg(windows)]
    #[test]
    fn missing_profiles_and_corrupt_ciphertext_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("models.json");
        let store = ModelProfileStore::new(target.clone()).unwrap();
        let error = store.resolve_secret("missing").unwrap_err();
        assert_eq!(error.code, "MODEL_PROFILE_NOT_FOUND");

        let mut document = StoredDocument::default();
        document.profiles.push(StoredProfile {
            id: "corrupt".into(),
            provider_name: "Provider".into(),
            note: None,
            website_url: None,
            base_url: "https://provider.example".into(),
            model_id: "model".into(),
            selected: false,
            protected_api_key: ProtectedValue {
                scheme: WINDOWS_DPAPI_SCHEME.into(),
                ciphertext: "00".into(),
            },
        });
        fs::write(&target, serde_json::to_vec(&document).unwrap()).unwrap();
        let error = store.resolve_secret("corrupt").unwrap_err();
        assert!(matches!(
            error.code.as_str(),
            "MODEL_SECRET_UNAVAILABLE" | "MODEL_SECRET_INVALID"
        ));
    }
}
