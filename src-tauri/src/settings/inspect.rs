use std::{path::Path, sync::Arc};

use serde_json::{Map, Value};

use crate::dto::{
    ApiError, ApiResult, ModelCandidate, ModelCandidateSource, ModelStatus, SkillOverrideState,
};

use super::SettingsTransaction;

#[derive(Clone)]
pub struct SettingsService {
    transaction: Arc<SettingsTransaction>,
}

impl SettingsService {
    pub fn new(transaction: SettingsTransaction) -> Self {
        Self {
            transaction: Arc::new(transaction),
        }
    }

    pub fn model_status(&self, project_root: Option<&Path>) -> ApiResult<ModelStatus> {
        let document = self.transaction.read()?;
        let desired_user_model = string_at(&document.object, "model");
        let mut candidates = Vec::new();
        let mut warnings = Vec::new();

        if let Some(model) = nested_env_model(&document.object) {
            candidates.push(ModelCandidate {
                source: ModelCandidateSource::UserEnv,
                label: "用户 settings.json 中的 env.ANTHROPIC_MODEL".into(),
                value: model,
                enforced: false,
            });
        }
        if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
            if valid_display_value(&model) {
                candidates.push(ModelCandidate {
                    source: ModelCandidateSource::ProcessEnv,
                    label: "CC Panel 进程环境 ANTHROPIC_MODEL".into(),
                    value: model,
                    enforced: false,
                });
            }
        }
        if let Some(root) = project_root {
            inspect_project_model(
                &root.join(".claude").join("settings.json"),
                ModelCandidateSource::Project,
                "项目 .claude/settings.json",
                &mut candidates,
                &mut warnings,
            );
            inspect_project_model(
                &root.join(".claude").join("settings.local.json"),
                ModelCandidateSource::ProjectLocal,
                "项目 .claude/settings.local.json",
                &mut candidates,
                &mut warnings,
            );
        }

        warnings.push("无法观察其他 Claude Code 进程中的 /model、--model 或 --settings。".into());
        Ok(ModelStatus {
            desired_user_model,
            settings_revision: document.revision,
            candidates,
            active_session_observable: false,
            warnings,
        })
    }

    pub fn set_user_model(
        &self,
        model: &str,
        expected_revision: &str,
        project_root: Option<&Path>,
    ) -> ApiResult<ModelStatus> {
        validate_model(model)?;
        self.transaction.update(expected_revision, |object| {
            object.insert("model".into(), Value::String(model.into()));
            Ok(())
        })?;
        self.model_status(project_root)
    }

    pub fn clear_user_model(
        &self,
        expected_revision: &str,
        project_root: Option<&Path>,
    ) -> ApiResult<ModelStatus> {
        self.transaction.update(expected_revision, |object| {
            object.remove("model");
            Ok(())
        })?;
        self.model_status(project_root)
    }

    pub fn skill_override(
        &self,
        canonical_id: &str,
    ) -> ApiResult<(SkillOverrideState, Option<String>, bool)> {
        let document = self.transaction.read()?;
        Ok(skill_override_from(&document.object, canonical_id))
    }

    pub fn settings_revision(&self) -> ApiResult<String> {
        Ok(self.transaction.read()?.revision)
    }

    pub fn all_skill_overrides(&self) -> ApiResult<(Map<String, Value>, String)> {
        let document = self.transaction.read()?;
        let map = document
            .object
            .get("skillOverrides")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Ok((map, document.revision))
    }

    pub fn set_skill_override(
        &self,
        canonical_id: &str,
        value: &str,
        expected_revision: &str,
    ) -> ApiResult<String> {
        validate_skill_id(canonical_id)?;
        if !matches!(
            value,
            "default" | "on" | "name-only" | "user-invocable-only" | "off"
        ) {
            return Err(ApiError::new(
                "INVALID_SKILL_OVERRIDE",
                "Skill 状态必须是继承、开启、仅名称、仅手动或关闭。",
                false,
            )
            .field("value"));
        }
        self.transaction.update(expected_revision, |object| {
            let overrides = object
                .entry("skillOverrides")
                .or_insert_with(|| Value::Object(Map::new()));
            let map = overrides.as_object_mut().ok_or_else(|| {
                ApiError::new(
                    "INVALID_SKILL_OVERRIDES_OBJECT",
                    "settings.json 中的 skillOverrides 不是对象；未做任何修改。",
                    false,
                )
            })?;
            if value == "default" {
                map.remove(canonical_id);
                if map.is_empty() {
                    object.remove("skillOverrides");
                }
            } else {
                map.insert(canonical_id.into(), Value::String(value.into()));
            }
            Ok(())
        })
    }
}

pub fn skill_override_from(
    settings: &Map<String, Value>,
    canonical_id: &str,
) -> (SkillOverrideState, Option<String>, bool) {
    let raw = settings
        .get("skillOverrides")
        .and_then(Value::as_object)
        .and_then(|map| map.get(canonical_id));
    let Some(raw) = raw else {
        return (SkillOverrideState::Default, None, false);
    };
    let raw_string = raw
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| raw.to_string());
    let state = match raw.as_str() {
        Some("on") => SkillOverrideState::On,
        Some("name-only") => SkillOverrideState::NameOnly,
        Some("user-invocable-only") => SkillOverrideState::UserInvocableOnly,
        Some("off") => SkillOverrideState::Off,
        _ => SkillOverrideState::Unknown,
    };
    (state, Some(raw_string), true)
}

fn validate_model(model: &str) -> ApiResult<()> {
    let bytes = model.len();
    if bytes == 0 || bytes > 512 || model.trim() != model || model.chars().any(char::is_control) {
        return Err(ApiError::new(
            "INVALID_MODEL_ID",
            "模型 ID 必须为 1–512 字节，且不能包含控制字符或首尾空白。",
            false,
        )
        .field("model"));
    }
    Ok(())
}

fn validate_skill_id(id: &str) -> ApiResult<()> {
    if id.is_empty()
        || id.len() > 256
        || id.trim() != id
        || id.starts_with('/')
        || id.contains("..")
        || id.contains('*')
        || id.contains(',')
        || id.contains('(')
        || id.contains(')')
        || id.chars().any(char::is_control)
    {
        return Err(ApiError::new(
            "INVALID_SKILL_ID",
            "Skill 标识无效。请刷新 Skill 清单后重试。",
            false,
        )
        .field("canonicalId"));
    }
    Ok(())
}

fn string_at(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn nested_env_model(object: &Map<String, Value>) -> Option<String> {
    object
        .get("env")
        .and_then(Value::as_object)
        .and_then(|env| env.get("ANTHROPIC_MODEL"))
        .and_then(Value::as_str)
        .filter(|value| valid_display_value(value))
        .map(ToOwned::to_owned)
}

fn valid_display_value(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn inspect_project_model(
    path: &Path,
    source: ModelCandidateSource,
    label: &str,
    candidates: &mut Vec<ModelCandidate>,
    warnings: &mut Vec<String>,
) {
    if !path.is_file() {
        return;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        warnings.push(format!("无法读取 {label}。"));
        return;
    };
    if metadata.len() > 4 * 1024 * 1024 {
        warnings.push(format!("{label} 过大，已跳过模型诊断。"));
        return;
    }
    let Ok(bytes) = std::fs::read(path) else {
        warnings.push(format!("无法读取 {label}。"));
        return;
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(object)) => {
            if let Some(value) = string_at(&object, "model") {
                if valid_display_value(&value) {
                    candidates.push(ModelCandidate {
                        source,
                        label: label.into(),
                        value,
                        enforced: false,
                    });
                }
            }
        }
        _ => warnings.push(format!("{label} 不是有效 JSON 对象，无法诊断模型。")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn custom_model_is_preserved_exactly() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("settings.json");
        fs::write(&target, b"{}").unwrap();
        let service = SettingsService::new(SettingsTransaction::new(
            target.clone(),
            temp.path().join("lock"),
        ));
        let revision = service.settings_revision().unwrap();
        let status = service
            .set_user_model("deepseek-v4-pro[1m]", &revision, None)
            .unwrap();
        assert_eq!(
            status.desired_user_model.as_deref(),
            Some("deepseek-v4-pro[1m]")
        );
    }

    #[test]
    fn default_removes_only_requested_override() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("settings.json");
        fs::write(
            &target,
            br#"{"skillOverrides":{"a":"off","b":"name-only"},"future":true}"#,
        )
        .unwrap();
        let service = SettingsService::new(SettingsTransaction::new(
            target.clone(),
            temp.path().join("lock"),
        ));
        let revision = service.settings_revision().unwrap();
        service
            .set_skill_override("a", "default", &revision)
            .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(target).unwrap()).unwrap();
        assert!(value["skillOverrides"].get("a").is_none());
        assert_eq!(value["skillOverrides"]["b"], "name-only");
        assert_eq!(value["future"], true);
    }
}
