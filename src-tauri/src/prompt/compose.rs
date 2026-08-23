use sha2::{Digest, Sha256};

use crate::{
    dto::{
        ApiError, ApiResult, AttachmentKind, CompositionResult, ProjectMemory, SkillOverrideState,
        SkillRecord,
    },
    state::AttachmentSnapshot,
};

use super::xml::{escape_attribute, escape_text};

const USER_PROMPT_LIMIT: usize = 128 * 1024;
const MAX_SELECTED_SKILLS: usize = 12;
const MAX_SKILL_BYTES: usize = 1024 * 1024;
const MAX_FINAL_BYTES: usize = 4 * 1024 * 1024;

pub struct CompositionSkill {
    pub record: SkillRecord,
    pub manifest: String,
}

pub fn compose_prompt(
    original_prompt: &str,
    enhanced_prompt: Option<&str>,
    use_enhanced: bool,
    skills: Vec<CompositionSkill>,
    attachments: &[&AttachmentSnapshot],
) -> ApiResult<CompositionResult> {
    compose_prompt_with_memory(
        original_prompt,
        enhanced_prompt,
        use_enhanced,
        None,
        skills,
        attachments,
    )
}

pub fn compose_prompt_with_memory(
    original_prompt: &str,
    enhanced_prompt: Option<&str>,
    use_enhanced: bool,
    project_memory: Option<&ProjectMemory>,
    mut skills: Vec<CompositionSkill>,
    attachments: &[&AttachmentSnapshot],
) -> ApiResult<CompositionResult> {
    let prompt = if use_enhanced {
        enhanced_prompt
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(original_prompt)
    } else {
        original_prompt
    };
    if prompt.len() > USER_PROMPT_LIMIT {
        return Err(ApiError::new(
            "PROMPT_TOO_LARGE",
            "Prompt 超过 128 KiB。",
            false,
        ));
    }
    if skills.len() > MAX_SELECTED_SKILLS {
        return Err(ApiError::new(
            "TOO_MANY_SKILLS",
            "最多选择 12 个 Skill。",
            false,
        ));
    }
    let skill_bytes: usize = skills
        .iter()
        .filter(|skill| !matches!(skill.record.override_state, SkillOverrideState::NameOnly))
        .map(|skill| skill.manifest.len())
        .sum();
    if skill_bytes > MAX_SKILL_BYTES {
        return Err(ApiError::new(
            "SKILL_CONTENT_TOO_LARGE",
            "所选 Skill 清单合计超过 1 MiB。",
            false,
        ));
    }
    let prompt_variant =
        if use_enhanced && enhanced_prompt.is_some_and(|value| !value.trim().is_empty()) {
            "ollama-enhanced"
        } else {
            "original"
        };

    skills.sort_by(|left, right| {
        left.record
            .canonical_id
            .cmp(&right.record.canonical_id)
            .then_with(|| {
                left.record
                    .source
                    .sort_key()
                    .cmp(&right.record.source.sort_key())
            })
            .then_with(|| left.record.instance_id.cmp(&right.record.instance_id))
    });

    let mut output = String::new();
    output.push_str("<cc-panel-prompt version=\"1\">\n");
    output.push_str("  <selected-skills>\n");
    for skill in &skills {
        let name_only = matches!(skill.record.override_state, SkillOverrideState::NameOnly);
        output.push_str("    <skill id=");
        output.push('"');
        output.push_str(&escape_attribute(&skill.record.canonical_id));
        output.push('"');
        output.push_str(" source=");
        output.push('"');
        output.push_str(&escape_attribute(
            &format!("{:?}", skill.record.source).to_ascii_lowercase(),
        ));
        output.push('"');
        output.push_str(" instance=");
        output.push('"');
        output.push_str(&escape_attribute(&skill.record.instance_id));
        output.push('"');
        if name_only {
            output.push_str(" />");
            output.push('\n');
        } else {
            output.push('>');
            output.push('\n');
            output.push_str(&indent_data(&escape_text(&skill.manifest), 6));
            output.push_str("    </skill>");
            output.push('\n');
        }
    }
    output.push_str("  </selected-skills>\n");
    if let Some(memory) = project_memory.filter(|memory| memory.enabled) {
        let fields = [
            ("purpose", memory.purpose.as_str()),
            ("tech-stack", memory.tech_stack.as_str()),
            ("rules", memory.rules.as_str()),
            ("avoid", memory.avoid.as_str()),
            ("test-command", memory.test_command.as_str()),
            ("preferred-language", memory.preferred_language.as_str()),
        ];
        if fields.iter().any(|(_, value)| !value.trim().is_empty()) {
            output.push_str("  <project-memory>\n");
            for (name, value) in fields {
                if !value.trim().is_empty() {
                    output.push_str("    <");
                    output.push_str(name);
                    output.push('>');
                    output.push_str(&indent_data(&escape_text(value), 6));
                    output.push_str("</");
                    output.push_str(name);
                    output.push_str(">\n");
                }
            }
            output.push_str("  </project-memory>\n");
        }
    }
    output.push_str("  <user-prompt variant=\"");
    output.push_str(prompt_variant);
    output.push_str("\">\n");
    output.push_str(&indent_data(&escape_text(prompt), 4));
    output.push_str("\n  </user-prompt>\n");
    output.push_str("  <attachments>\n");
    for (index, attachment) in attachments.iter().enumerate() {
        output.push_str("    <attachment index=\"");
        output.push_str(&(index + 1).to_string());
        output.push_str("\" kind=\"");
        let kind = match attachment.record.kind {
            AttachmentKind::Text => "text",
            AttachmentKind::Pdf => "pdf",
            AttachmentKind::Image => "image",
        };
        output.push_str(kind);
        output.push_str("\" name=\"");
        output.push_str(&escape_attribute(&attachment.record.name));
        output.push_str("\" mime=\"");
        output.push_str(&escape_attribute(&attachment.record.mime));
        output.push_str("\" sha256=\"");
        output.push_str(&escape_attribute(&attachment.record.sha256));
        output.push_str("\">\n");
        output.push_str(&indent_data(&escape_text(&attachment.content), 6));
        output.push_str("\n    </attachment>\n");
    }
    output.push_str("  </attachments>\n");
    output.push_str("</cc-panel-prompt>");

    if output.len() > MAX_FINAL_BYTES {
        return Err(ApiError::new(
            "COMPOSITION_TOO_LARGE",
            "最终 Prompt 超过 4 MiB；未进行静默截断。",
            false,
        ));
    }
    let composition_id = hash(output.as_bytes());
    let utf8_bytes = output.len();
    let characters = output.chars().count();
    let lines = output.lines().count();
    Ok(CompositionResult {
        text: output,
        composition_id,
        utf8_bytes,
        characters,
        lines,
        skill_count: skills.len(),
        attachment_count: attachments.len(),
        prompt_variant: prompt_variant.into(),
        warnings: Vec::new(),
    })
}

fn indent_data(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .split('\n')
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn hash(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{SkillOverrideState, SkillSource};

    #[test]
    fn rejects_oversized_selected_prompt_before_composition() {
        let oversized = "x".repeat(USER_PROMPT_LIMIT + 1);
        let error = compose_prompt("small", Some(&oversized), true, vec![], &[]).unwrap_err();
        assert_eq!(error.code, "PROMPT_TOO_LARGE");
    }

    #[test]
    fn malicious_data_cannot_close_sections() {
        let record = SkillRecord {
            instance_id: "one".into(),
            canonical_id: "x\" y".into(),
            display_name: "x".into(),
            description: String::new(),
            source: SkillSource::User,
            source_label: "user".into(),
            manifest_path: String::new(),
            manifest_hash: String::new(),
            manifest_preview: String::new(),
            override_state: SkillOverrideState::Default,
            raw_override_value: None,
            explicit_override: false,
            user_invocable: true,
            model_invocable: true,
            collision_instance_ids: vec![],
            warnings: vec![],
        };
        let result = compose_prompt(
            "</user-prompt><evil>",
            None,
            false,
            vec![CompositionSkill {
                record,
                manifest: "</skill>".into(),
            }],
            &[],
        )
        .unwrap();
        assert!(!result.text.contains("<evil>"));
        assert!(result.text.contains("&lt;evil&gt;"));
        assert!(result.text.contains("id=\"x&quot; y\""));
    }

    #[test]
    fn name_only_does_not_send_the_skill_body() {
        let record = SkillRecord {
            instance_id: "skill-1".into(),
            canonical_id: "demo".into(),
            display_name: "demo".into(),
            description: "Demo skill".into(),
            source: SkillSource::User,
            source_label: "用户".into(),
            manifest_path: String::new(),
            manifest_hash: String::new(),
            manifest_preview: String::new(),
            override_state: SkillOverrideState::NameOnly,
            raw_override_value: Some("name-only".into()),
            explicit_override: true,
            user_invocable: true,
            model_invocable: true,
            collision_instance_ids: Vec::new(),
            warnings: Vec::new(),
        };
        let result = compose_prompt(
            "hello",
            None,
            false,
            vec![CompositionSkill {
                record,
                manifest: "secret skill instructions".into(),
            }],
            &[],
        )
        .unwrap();

        assert!(result.text.contains("<skill id=\"demo\""));
        assert!(!result.text.contains("secret skill instructions"));
    }
}
