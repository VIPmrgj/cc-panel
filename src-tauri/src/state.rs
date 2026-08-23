use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, RwLock},
    time::Instant,
};

use tauri::{AppHandle, Manager};

use crate::{
    config::ConfigStore,
    conversations::ConversationIndex,
    dto::{AttachmentRecord, SkillInventory},
    model_profiles::ModelProfileStore,
    platform::ClaudePaths,
    sessions::SessionManager,
    settings::{SettingsService, SettingsTransaction},
    skills::{PluginCli, SkillScanner},
};

pub struct AttachmentSnapshot {
    pub record: AttachmentRecord,
    pub content: String,
    pub preview_bytes: Option<Vec<u8>>,
}

pub struct PendingAttachment {
    pub token: String,
    pub expires_at: Instant,
    pub snapshot: AttachmentSnapshot,
    pub reason: String,
}

#[derive(Debug)]
pub struct DroppedAttachmentGrant {
    pub expires_at: Instant,
    pub paths: Vec<PathBuf>,
}

pub struct AppState {
    pub paths: ClaudePaths,
    pub config: ConfigStore,
    pub model_profiles: ModelProfileStore,
    pub conversations: ConversationIndex,
    pub project_memory: crate::product::ProjectMemoryStore,
    pub sessions: SessionManager,
    pub settings: SettingsService,
    pub skill_scanner: SkillScanner,
    pub plugin_cli: PluginCli,
    pub ollama_client: crate::ollama::OllamaClient,
    pub ollama_enhancement: tokio::sync::Semaphore,
    pub skill_inventory: RwLock<Option<SkillInventory>>,
    pub attachments: Mutex<HashMap<String, AttachmentSnapshot>>,
    pub pending_attachments: Mutex<HashMap<String, PendingAttachment>>,
    pub dropped_attachment_grants: Mutex<HashMap<String, DroppedAttachmentGrant>>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let paths = ClaudePaths::detect()?;
        let config_dir = app.path().app_config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config = ConfigStore::load(config_dir.join("cc-panel.json"))?;
        let panel_home = paths.home().join(".cc-panel");
        ensure_private_directory(&panel_home)?;
        let model_profiles = ModelProfileStore::new(panel_home.join("models.json"))?;
        let conversations = ConversationIndex::load(panel_home.join("conversations.json"))?;
        let project_memory =
            crate::product::ProjectMemoryStore::new(panel_home.join("project-memory"))?;
        let transaction = SettingsTransaction::new(
            paths.user_settings().to_path_buf(),
            config_dir.join("claude-settings.lock"),
        );
        Ok(Self {
            paths,
            config,
            model_profiles,
            conversations,
            project_memory,
            sessions: SessionManager::default(),
            settings: SettingsService::new(transaction),
            skill_scanner: SkillScanner,
            plugin_cli: PluginCli,
            ollama_client: crate::ollama::OllamaClient::new()?,
            ollama_enhancement: tokio::sync::Semaphore::new(1),
            skill_inventory: RwLock::new(None),
            attachments: Mutex::new(HashMap::new()),
            pending_attachments: Mutex::new(HashMap::new()),
            dropped_attachment_grants: Mutex::new(HashMap::new()),
        })
    }

    pub fn project_root(&self) -> Option<PathBuf> {
        self.config
            .preferences()
            .selected_project_root
            .map(|entry| PathBuf::from(entry.path))
    }

    pub fn known_skill(&self, canonical_id: &str) -> bool {
        self.skill_inventory
            .read()
            .expect("skill inventory poisoned")
            .as_ref()
            .is_some_and(|inventory| {
                inventory
                    .skills
                    .iter()
                    .any(|skill| skill.canonical_id == canonical_id)
            })
    }

    pub fn attachment_records(&self) -> Vec<AttachmentRecord> {
        self.attachments
            .lock()
            .expect("attachment store poisoned")
            .values()
            .map(|snapshot| snapshot.record.clone())
            .collect()
    }
}

fn ensure_private_directory(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::other(
                "CC Panel data path is not a regular directory",
            ));
        }
    } else {
        std::fs::create_dir(path)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
