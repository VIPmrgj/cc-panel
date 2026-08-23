use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootEntry {
    pub id: String,
    pub path: String,
    pub label: String,
    pub kind: RootKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RootKind {
    Project,
    Additional,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCandidate {
    pub source: ModelCandidateSource,
    pub label: String,
    pub value: String,
    pub enforced: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelCandidateSource {
    ProcessEnv,
    UserEnv,
    Project,
    ProjectLocal,
    Managed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub desired_user_model: Option<String>,
    pub settings_revision: String,
    pub candidates: Vec<ModelCandidate>,
    pub active_session_observable: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSource {
    User,
    Project,
    Additional,
    Plugin,
}

impl SkillSource {
    pub fn sort_key(&self) -> u8 {
        match self {
            Self::Project => 0,
            Self::User => 1,
            Self::Additional => 2,
            Self::Plugin => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillOverrideState {
    Default,
    On,
    NameOnly,
    UserInvocableOnly,
    Off,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRecord {
    pub instance_id: String,
    pub canonical_id: String,
    pub display_name: String,
    pub description: String,
    pub source: SkillSource,
    pub source_label: String,
    pub manifest_path: String,
    pub manifest_hash: String,
    pub manifest_preview: String,
    pub override_state: SkillOverrideState,
    pub raw_override_value: Option<String>,
    pub explicit_override: bool,
    pub user_invocable: bool,
    pub model_invocable: bool,
    pub collision_instance_ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInventory {
    pub skills: Vec<SkillRecord>,
    pub settings_revision: String,
    pub claude_cli_available: bool,
    pub plugin_warning: Option<String>,
    pub scanned_at_revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    pub selected_project_root: Option<RootEntry>,
    pub additional_roots: Vec<RootEntry>,
    pub ollama_base_url: String,
    pub ollama_model: Option<String>,
    pub native_notifications_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModel {
    pub name: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub online: bool,
    pub base_url: String,
    pub selected_model: Option<String>,
    pub models: Vec<OllamaModel>,
    pub auto_selected: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub preferences: AppPreferences,
    pub model: ModelStatus,
    pub skills: SkillInventory,
    pub ollama: OllamaStatus,
    pub attachments: Vec<AttachmentRecord>,
    pub claude_code_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Text,
    Pdf,
    Image,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRecord {
    pub handle: String,
    pub name: String,
    pub kind: AttachmentKind,
    pub mime: String,
    pub raw_bytes: usize,
    pub extracted_bytes: usize,
    pub sha256: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPreview {
    pub attachment: AttachmentRecord,
    pub content: String,
    pub truncated: bool,
    pub data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSensitiveAttachment {
    pub confirmation_token: String,
    pub name: String,
    pub reason: String,
    pub raw_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedAttachment {
    pub name: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentImportResult {
    pub imported: Vec<AttachmentRecord>,
    pub pending_confirmation: Vec<PendingSensitiveAttachment>,
    pub rejected: Vec<RejectedAttachment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedSkillRequest {
    pub instance_id: String,
    pub manifest_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionRequest {
    pub original_prompt: String,
    pub enhanced_prompt: Option<String>,
    pub use_enhanced: bool,
    pub selected_skills: Vec<SelectedSkillRequest>,
    pub attachment_handles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionResult {
    pub text: String,
    pub composition_id: String,
    pub utf8_bytes: usize,
    pub characters: usize,
    pub lines: usize,
    pub skill_count: usize,
    pub attachment_count: usize,
    pub prompt_variant: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    #[serde(flatten)]
    pub composition: CompositionResult,
    pub copied: bool,
    pub notification_sent: bool,
    pub notification_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancePromptResponse {
    pub text: String,
    pub model: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub id: String,
    pub tool_name: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemory {
    pub project_path: String,
    pub enabled: bool,
    pub purpose: String,
    pub tech_stack: String,
    pub rules: String,
    pub avoid: String,
    pub test_command: String,
    pub preferred_language: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMemoryInput {
    pub enabled: bool,
    pub purpose: String,
    pub tech_stack: String,
    pub rules: String,
    pub avoid: String,
    pub test_command: String,
    pub preferred_language: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub summary: String,
    pub detail: String,
    pub fix_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub checked_at_ms: u64,
    pub checks: Vec<EnvironmentCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticResult {
    pub path: String,
    pub created_at_ms: u64,
    pub included_sections: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub installer_url: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedUpdate {
    pub path: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoRunResult {
    pub user_id: String,
    pub file_name: String,
    pub relative_path: String,
    pub content: String,
    pub created_at_ms: u64,
}
