use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ContextEnum, ContextFile};
use crate::ext::config_dirs::get_ext_dirs;
use crate::ext::skills::load_skill_full;
use crate::ext::skills_context::expand_skill_includes;
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType, json_schema_from_params};

pub struct ToolActivateSkill {
    pub config_path: String,
}

async fn activate_skill_inner(
    ext_dirs: &crate::ext::config_dirs::ExtDirs,
    name: &str,
) -> Result<(ContextFile, Vec<String>, Option<String>), String> {
    let skill = load_skill_full(ext_dirs, name).await
        .ok_or_else(|| format!("Skill '{}' not found", name))?;
    if !skill.index.user_invocable {
        return Err(format!("Skill '{}' is not available for activation", name));
    }
    let body = expand_skill_includes(&skill.body, &skill.skill_dir).await;
    let line_count = body.lines().count().max(1);
    let cf = ContextFile {
        file_name: format!("skill://{}", name),
        file_content: body,
        line1: 1,
        line2: line_count,
        file_rev: None,
        symbols: vec![],
        gradient_type: 0,
        usefulness: 90.0,
        skip_pp: true,
    };
    Ok((cf, skill.allowed_tools, skill.model))
}

#[async_trait]
impl Tool for ToolActivateSkill {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "activate_skill".to_string(),
            display_name: "Activate Skill".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: true,
            description: "Load a skill's full instructions into the current context. Use when you determine a skill from the available index is relevant to the user's request.".to_string(),
            input_schema: json_schema_from_params(
                &[("name", "string", "Name of the skill to activate")],
                &["name"],
            ),
            output_schema: None,
            annotations: None,
        }
    }

    async fn tool_execute(
        &mut self,
        ccx: Arc<AMutex<AtCommandsContext>>,
        _tool_call_id: &String,
        args: &HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>), String> {
        let name = match args.get("name") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => return Err(format!("argument `name` is not a string: {:?}", v)),
            None => return Err("argument `name` is missing".to_string()),
        };

        let (gcx, chat_id) = {
            let ccx_locked = ccx.lock().await;
            (ccx_locked.global_context.clone(), ccx_locked.chat_id.clone())
        };
        let ext_dirs = get_ext_dirs(gcx.clone()).await;
        let (context_file, allowed_tools, model_override) = activate_skill_inner(&ext_dirs, &name).await?;

        {
            let session_arc_opt = {
                let gcx_locked = gcx.read().await;
                let sessions = gcx_locked.chat_sessions.read().await;
                sessions.get(&chat_id).cloned()
            };
            if let Some(session_arc) = session_arc_opt {
                let mut session = session_arc.lock().await;
                session.active_command.name = name.clone();
                session.active_command.allowed_tools = allowed_tools;
                session.active_command.model_override = model_override;
            }
        }

        Ok((false, vec![ContextEnum::ContextFile(context_file)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext::config_dirs::ExtDirs;
    use std::path::Path;

    fn make_ext_dirs(root: &Path) -> ExtDirs {
        ExtDirs {
            global_dirs: vec![root.to_path_buf()],
            installed_dirs: vec![],
            project_dirs: vec![],
        }
    }

    async fn write_skill(root: &Path, name: &str, frontmatter: &str, body: &str) {
        let skill_dir = root.join("skills").join(name);
        tokio::fs::create_dir_all(&skill_dir).await.unwrap();
        let content = format!("---\n{}\n---\n{}", frontmatter, body);
        tokio::fs::write(skill_dir.join("SKILL.md"), content).await.unwrap();
    }

    #[tokio::test]
    async fn test_activate_known_skill() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "my-skill",
            "name: my-skill\ndescription: A useful skill\nuser-invocable: true",
            "Do something useful with $ARGUMENTS",
        )
        .await;

        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "my-skill").await;
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (cf, allowed_tools, model_override) = result.unwrap();
        assert_eq!(cf.file_name, "skill://my-skill");
        assert!(cf.file_content.contains("Do something useful with $ARGUMENTS"));
        assert_eq!(cf.line1, 1);
        assert!(cf.skip_pp);
        assert!(allowed_tools.is_empty());
        assert!(model_override.is_none());
    }

    #[tokio::test]
    async fn test_activate_unknown_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "nonexistent").await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("not found"), "Expected 'not found' in error: {}", msg);
    }

    #[tokio::test]
    async fn test_activate_non_invocable_skill() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "hidden-skill",
            "name: hidden-skill\ndescription: Internal skill\nuser-invocable: false",
            "Internal instructions",
        )
        .await;

        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "hidden-skill").await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not available for activation"),
            "Expected 'not available for activation' in error: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_activate_skill_with_includes() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("skills").join("with-include");
        tokio::fs::create_dir_all(&skill_dir).await.unwrap();
        tokio::fs::write(skill_dir.join("context.md"), "Included content here").await.unwrap();
        tokio::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: with-include\ndescription: Skill with includes\nuser-invocable: true\n---\nBefore\n@include context.md\nAfter",
        )
        .await
        .unwrap();

        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "with-include").await;
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (cf, _, _) = result.unwrap();
        assert!(
            cf.file_content.contains("Included content here"),
            "@include should be expanded, got: {}",
            cf.file_content
        );
        assert!(!cf.file_content.contains("@include"), "@include directive should be replaced");
    }

    #[tokio::test]
    async fn test_activate_skill_returns_allowed_tools_and_model() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "restricted-skill",
            "name: restricted-skill\ndescription: Skill with restrictions\nuser-invocable: true\nallowed-tools:\n  - cat\n  - tree\nmodel: gpt-4o",
            "Do something restricted",
        )
        .await;

        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "restricted-skill").await;
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let (cf, allowed_tools, model_override) = result.unwrap();
        assert_eq!(cf.file_name, "skill://restricted-skill");
        assert_eq!(allowed_tools, vec!["cat".to_string(), "tree".to_string()]);
        assert_eq!(model_override, Some("gpt-4o".to_string()));
    }

    #[tokio::test]
    async fn test_activate_skill_empty_allowed_tools() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "open-skill",
            "name: open-skill\ndescription: Skill without restrictions\nuser-invocable: true",
            "Do anything",
        )
        .await;

        let ext_dirs = make_ext_dirs(tmp.path());
        let result = activate_skill_inner(&ext_dirs, "open-skill").await;
        assert!(result.is_ok());
        let (_, allowed_tools, model_override) = result.unwrap();
        assert!(allowed_tools.is_empty(), "No restrictions should result in empty allowed_tools");
        assert!(model_override.is_none(), "No model should result in None model_override");
    }
}
