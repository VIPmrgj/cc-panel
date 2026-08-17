use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    dto::{ApiError, ApiResult, SkillInventory, SkillOverrideState, SkillRecord, SkillSource},
    settings::skill_override_from,
};

use super::PluginRoot;

const MANIFEST_LIMIT: u64 = 256 * 1024;
const PREVIEW_LIMIT_CHARS: usize = 12_000;

#[derive(Debug, Clone)]
pub struct SkillRoot {
    pub directory: PathBuf,
    pub source: SkillSource,
    pub source_label: String,
    pub plugin_name: Option<String>,
}

impl SkillRoot {
    pub fn user(directory: PathBuf) -> Self {
        Self {
            directory,
            source: SkillSource::User,
            source_label: "用户".into(),
            plugin_name: None,
        }
    }

    pub fn project(project: &Path) -> Self {
        Self {
            directory: project.join(".claude").join("skills"),
            source: SkillSource::Project,
            source_label: "当前项目".into(),
            plugin_name: None,
        }
    }

    pub fn additional(base: &Path, label: &str) -> Self {
        Self {
            directory: base.join(".claude").join("skills"),
            source: SkillSource::Additional,
            source_label: format!("附加目录 · {label}"),
            plugin_name: None,
        }
    }

    pub fn plugin(plugin: PluginRoot) -> Self {
        Self {
            directory: plugin.install_path.join("skills"),
            source: SkillSource::Plugin,
            source_label: format!("插件 · {}", plugin.plugin_id),
            plugin_name: Some(plugin.plugin_name),
        }
    }
}

#[derive(Default, Clone)]
pub struct SkillScanner;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
    user_invocable: Option<bool>,
    disable_model_invocation: Option<bool>,
}

impl SkillScanner {
    pub fn scan(
        &self,
        roots: &[SkillRoot],
        overrides: &Map<String, Value>,
        settings_revision: String,
        claude_cli_available: bool,
        plugin_warning: Option<String>,
    ) -> SkillInventory {
        let mut skills = Vec::new();
        for root in roots {
            self.scan_root(root, overrides, &mut skills);
        }
        mark_collisions(&mut skills);
        skills.sort_by(|left, right| {
            left.source
                .sort_key()
                .cmp(&right.source.sort_key())
                .then_with(|| left.canonical_id.cmp(&right.canonical_id))
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        let mut digest = Sha256::new();
        for skill in &skills {
            digest.update(skill.instance_id.as_bytes());
            digest.update(skill.manifest_hash.as_bytes());
        }
        SkillInventory {
            skills,
            settings_revision,
            claude_cli_available,
            plugin_warning,
            scanned_at_revision: hex::encode(digest.finalize()),
        }
    }

    fn scan_root(
        &self,
        root: &SkillRoot,
        overrides: &Map<String, Value>,
        output: &mut Vec<SkillRecord>,
    ) {
        let Ok(canonical_root) = dunce::canonicalize(&root.directory) else {
            return;
        };
        let Ok(entries) = fs::read_dir(&canonical_root) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let directory_name = entry.file_name().to_string_lossy().into_owned();
            if directory_name.is_empty() {
                continue;
            }
            let manifest = match fs::read_dir(entry.path()) {
                Ok(entries) => entries
                    .flatten()
                    .find(|candidate| candidate.file_name() == "SKILL.md")
                    .map(|candidate| candidate.path()),
                Err(_) => None,
            };
            let Some(manifest) = manifest.filter(|path| path.is_file()) else {
                continue;
            };
            let canonical_manifest = match dunce::canonicalize(&manifest) {
                Ok(path) if path.starts_with(&canonical_root) => path,
                _ => continue,
            };
            match read_skill(root, &directory_name, &canonical_manifest, overrides) {
                Ok(skill) => output.push(skill),
                Err(error) => output.push(error_record(
                    root,
                    &directory_name,
                    &canonical_manifest,
                    error.message,
                    overrides,
                )),
            }
        }
    }
}

fn read_skill(
    root: &SkillRoot,
    directory_name: &str,
    manifest: &Path,
    overrides: &Map<String, Value>,
) -> ApiResult<SkillRecord> {
    let metadata = fs::metadata(manifest).map_err(|_| ApiError::io("inspect-skill-manifest"))?;
    if metadata.len() > MANIFEST_LIMIT {
        return Err(ApiError::new(
            "SKILL_MANIFEST_TOO_LARGE",
            "SKILL.md 超过 256 KiB，已跳过正文读取。",
            false,
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(manifest)
        .map_err(|_| ApiError::io("open-skill-manifest"))?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ApiError::io("read-skill-manifest"))?;
    let content = String::from_utf8(bytes.clone()).map_err(|_| {
        ApiError::new(
            "SKILL_MANIFEST_INVALID_UTF8",
            "SKILL.md 不是有效 UTF-8。",
            false,
        )
    })?;
    let (frontmatter, mut warnings) = parse_frontmatter(&content)?;
    if frontmatter.name.as_deref() != Some(directory_name) {
        warnings.push("frontmatter name 与目录名不同；运行时 ID 仍使用目录名。".into());
    }
    let canonical_id = match &root.plugin_name {
        Some(plugin) => format!("{plugin}:{directory_name}"),
        None => directory_name.to_owned(),
    };
    let (override_state, raw_override_value, explicit_override) =
        skill_override_from(overrides, &canonical_id);
    let model_invocable = !frontmatter.disable_model_invocation.unwrap_or(false)
        && !matches!(
            override_state,
            SkillOverrideState::UserInvocableOnly | SkillOverrideState::Off
        );
    let user_invocable = frontmatter.user_invocable.unwrap_or(true)
        && !matches!(override_state, SkillOverrideState::Off);
    let manifest_hash = hash_bytes(&bytes);
    let instance_id = instance_id(root, &canonical_id, manifest);
    Ok(SkillRecord {
        instance_id,
        canonical_id,
        display_name: frontmatter.name.unwrap_or_else(|| directory_name.into()),
        description: frontmatter.description.unwrap_or_default(),
        source: root.source.clone(),
        source_label: root.source_label.clone(),
        manifest_path: manifest.to_string_lossy().into_owned(),
        manifest_hash,
        manifest_preview: preview_text(&content),
        override_state,
        raw_override_value,
        explicit_override,
        user_invocable,
        model_invocable,
        collision_instance_ids: Vec::new(),
        warnings,
    })
}

fn parse_frontmatter(content: &str) -> ApiResult<(Frontmatter, Vec<String>)> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Err(ApiError::new(
            "SKILL_FRONTMATTER_MISSING",
            "SKILL.md 缺少 YAML frontmatter；已禁用调用。",
            false,
        ));
    }
    let normalized = content.replace("\r\n", "\n");
    let after_open = &normalized[4..];
    let Some(end) = after_open.find("\n---\n") else {
        return Err(ApiError::new(
            "SKILL_FRONTMATTER_UNCLOSED",
            "SKILL.md 的 YAML frontmatter 未正确闭合；已禁用调用。",
            false,
        ));
    };
    serde_yaml::from_str::<Frontmatter>(&after_open[..end])
        .map(|frontmatter| (frontmatter, Vec::new()))
        .map_err(|_| {
            ApiError::new(
                "SKILL_FRONTMATTER_INVALID",
                "SKILL.md 的 YAML frontmatter 无法解析；已禁用调用。",
                false,
            )
        })
}

fn preview_text(content: &str) -> String {
    let mut preview = content
        .chars()
        .take(PREVIEW_LIMIT_CHARS)
        .collect::<String>();
    if content.chars().count() > PREVIEW_LIMIT_CHARS {
        preview.push_str("\n\n[预览已折叠；组合时使用完整清单]");
    }
    preview
}

fn error_record(
    root: &SkillRoot,
    directory_name: &str,
    manifest: &Path,
    warning: String,
    overrides: &Map<String, Value>,
) -> SkillRecord {
    let canonical_id = root
        .plugin_name
        .as_ref()
        .map(|plugin| format!("{plugin}:{directory_name}"))
        .unwrap_or_else(|| directory_name.into());
    let (override_state, raw_override_value, explicit_override) =
        skill_override_from(overrides, &canonical_id);
    SkillRecord {
        instance_id: instance_id(root, &canonical_id, manifest),
        canonical_id,
        display_name: directory_name.into(),
        description: String::new(),
        source: root.source.clone(),
        source_label: root.source_label.clone(),
        manifest_path: manifest.to_string_lossy().into_owned(),
        manifest_hash: String::new(),
        manifest_preview: String::new(),
        override_state,
        raw_override_value,
        explicit_override,
        user_invocable: false,
        model_invocable: false,
        collision_instance_ids: Vec::new(),
        warnings: vec![warning],
    }
}

fn instance_id(root: &SkillRoot, canonical_id: &str, manifest: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(format!("{:?}", root.source).as_bytes());
    digest.update(canonical_id.as_bytes());
    digest.update(manifest.to_string_lossy().as_bytes());
    format!("skill-{}", &hex::encode(digest.finalize())[..24])
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}

fn mark_collisions(skills: &mut [SkillRecord]) {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for skill in skills.iter() {
        groups
            .entry(skill.canonical_id.clone())
            .or_default()
            .push(skill.instance_id.clone());
    }
    for skill in skills.iter_mut() {
        if let Some(instances) = groups.get(&skill.canonical_id) {
            if instances.len() > 1 {
                skill.collision_instance_ids = instances
                    .iter()
                    .filter(|id| *id != &skill.instance_id)
                    .cloned()
                    .collect();
                skill.warnings.push(
                    "多个来源使用相同 ID；CC Panel 不断言另一个会话最终解析了哪一个。".into(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_exact_uppercase_manifest_only() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("skills");
        fs::create_dir_all(root.join("good")).unwrap();
        fs::create_dir_all(root.join("wrong")).unwrap();
        fs::write(
            root.join("good/SKILL.md"),
            "---\nname: good\ndescription: ok\n---\nBody",
        )
        .unwrap();
        fs::write(root.join("wrong/skill.md"), "---\nname: wrong\n---\nBody").unwrap();
        let inventory = SkillScanner.scan(
            &[SkillRoot::user(root)],
            &Map::new(),
            "revision".into(),
            false,
            None,
        );
        assert_eq!(inventory.skills.len(), 1);
        assert_eq!(inventory.skills[0].canonical_id, "good");
    }

    #[test]
    fn flags_collisions_without_dropping_instances() {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user");
        let project = temp.path().join("project/.claude/skills");
        for root in [&user, &project] {
            fs::create_dir_all(root.join("same")).unwrap();
            fs::write(
                root.join("same/SKILL.md"),
                "---\nname: same\ndescription: ok\n---\nBody",
            )
            .unwrap();
        }
        let inventory = SkillScanner.scan(
            &[
                SkillRoot::user(user),
                SkillRoot {
                    directory: project,
                    source: SkillSource::Project,
                    source_label: "project".into(),
                    plugin_name: None,
                },
            ],
            &Map::new(),
            "revision".into(),
            false,
            None,
        );
        assert_eq!(inventory.skills.len(), 2);
        assert!(inventory
            .skills
            .iter()
            .all(|skill| !skill.collision_instance_ids.is_empty()));
    }
}
