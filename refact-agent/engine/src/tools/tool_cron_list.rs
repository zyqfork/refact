use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::files_correction::get_active_project_path;
use crate::scheduler::{human_schedule, next_run_ms, CronStore, JsonFileCronStore, ScheduledTask};
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType};

pub struct ToolCronList {
    pub config_path: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CronListScope {
    Session,
    Durable,
    All,
}

#[async_trait]
impl Tool for ToolCronList {
    async fn tool_execute(
        &mut self,
        ccx: Arc<AMutex<AtCommandsContext>>,
        tool_call_id: &String,
        args: &HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>), String> {
        let scope = parse_scope(args)?;
        let gcx = ccx.lock().await.app.gcx.clone();
        let project_root = get_active_project_path(gcx)
            .await
            .ok_or_else(|| "No active project for scheduled tasks".to_string())?;
        let store = JsonFileCronStore::new(project_root)?;
        let now_ms = Utc::now().timestamp_millis().max(0) as u64;
        let tasks = store
            .list()
            .await
            .into_iter()
            .filter(|task| matches_scope(task, scope))
            .map(|task| task_value(&task, now_ms))
            .collect::<Vec<_>>();
        let text = serde_json::to_string_pretty(&tasks).map_err(|error| error.to_string())?;

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(text),
                tool_calls: None,
                tool_call_id: tool_call_id.clone(),
                ..Default::default()
            })],
        ))
    }

    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "cron_list".to_string(),
            display_name: "Cron List".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: true,
            description:
                "List scheduled tasks, optionally filtering by session-only or durable scope."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["session", "durable", "all"],
                        "default": "all"
                    }
                },
                "required": []
            }),
            output_schema: None,
            annotations: None,
        }
    }

    fn tool_depends_on(&self) -> Vec<String> {
        vec![]
    }

    fn has_config_path(&self) -> Option<String> {
        Some(self.config_path.clone())
    }
}

fn parse_scope(args: &HashMap<String, Value>) -> Result<CronListScope, String> {
    match args.get("scope") {
        Some(Value::String(scope)) if scope.trim().is_empty() => Ok(CronListScope::All),
        Some(Value::String(scope)) => match scope.trim() {
            "session" => Ok(CronListScope::Session),
            "durable" => Ok(CronListScope::Durable),
            "all" => Ok(CronListScope::All),
            other => Err(format!(
                "Invalid scope `{other}`. Must be one of: session, durable, all"
            )),
        },
        Some(value) => Err(format!("argument `scope` is not a string: {value:?}")),
        None => Ok(CronListScope::All),
    }
}

fn matches_scope(task: &ScheduledTask, scope: CronListScope) -> bool {
    match scope {
        CronListScope::Session => !task.durable,
        CronListScope::Durable => task.durable,
        CronListScope::All => true,
    }
}

fn task_value(task: &ScheduledTask, now_ms: u64) -> Value {
    json!({
        "id": task.id,
        "cron": task.cron,
        "human_schedule": human_schedule(&task.cron),
        "description": task.description,
        "prompt": first_chars(&task.prompt, 200),
        "recurring": task.recurring,
        "durable": task.durable,
        "next_fire_at_ms": next_run_ms(&task.cron, now_ms, chrono_tz::UTC).unwrap_or(0),
        "fire_count": task.fire_count,
        "created_at_ms": task.created_at_ms,
    })
}

fn first_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;

    fn task(id: &str, durable: bool, prompt: &str) -> ScheduledTask {
        ScheduledTask {
            id: id.to_string(),
            cron: "*/5 * * * *".to_string(),
            prompt: prompt.to_string(),
            description: format!("{id} description"),
            recurring: true,
            durable,
            created_at_ms: 123,
            chat_id: Some("chat".to_string()),
            mode: Some("agent".to_string()),
            last_fired_at_ms: None,
            fire_count: if durable { 2 } else { 1 },
            auto_expire_after_ms: crate::scheduler::DEFAULT_RECURRING_AUTO_EXPIRE_AFTER_MS,
        }
    }

    async fn ccx(project_root: std::path::PathBuf) -> Arc<AMutex<AtCommandsContext>> {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        {
            *gcx.documents_state.workspace_folders.lock().unwrap() = vec![project_root.clone()];
            gcx.documents_state
                .active_file_path
                .lock()
                .await
                .replace(project_root.join("src/lib.rs"));
        }
        let ccx = AtCommandsContext::new_with_abort(
            AppState::from_gcx(gcx).await,
            4096,
            20,
            false,
            Vec::new(),
            "chat".to_string(),
            None,
            "model".to_string(),
            None,
            None,
            None,
        )
        .await;
        Arc::new(AMutex::new(ccx))
    }

    async fn run_tool(
        tool: &mut ToolCronList,
        ccx: Arc<AMutex<AtCommandsContext>>,
        scope: Option<&str>,
    ) -> Vec<Value> {
        let mut args = HashMap::new();
        if let Some(scope) = scope {
            args.insert("scope".to_string(), json!(scope));
        }
        let (_, messages) = tool
            .tool_execute(ccx, &"call".to_string(), &args)
            .await
            .unwrap();
        let message = match messages.into_iter().next().unwrap() {
            ContextEnum::ChatMessage(message) => message,
            ContextEnum::ContextFile(_) => panic!("expected chat message"),
        };
        let text = match message.content {
            ChatContent::SimpleText(text) => text,
            _ => panic!("expected text content"),
        };
        serde_json::from_str(&text).unwrap()
    }

    #[tokio::test]
    async fn lists_all_and_filters_by_scope() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        let store = JsonFileCronStore::new(temp.path()).unwrap();
        let long_prompt = "x".repeat(250);
        store
            .add(task("cron_session", false, &long_prompt))
            .await
            .unwrap();
        store
            .add(task("cron_durable", true, "durable prompt"))
            .await
            .unwrap();

        let ccx = ccx(temp.path().to_path_buf()).await;
        let mut tool = ToolCronList {
            config_path: String::new(),
        };

        let all = run_tool(&mut tool, ccx.clone(), None).await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["id"], json!("cron_durable"));
        assert_eq!(all[0]["human_schedule"], json!("every 5 minutes"));
        assert_eq!(all[0]["description"], json!("cron_durable description"));
        assert!(all[0]["next_fire_at_ms"].as_u64().unwrap() > 0);
        assert_eq!(all[0]["fire_count"], json!(2));
        assert_eq!(all[0]["created_at_ms"], json!(123));
        assert_eq!(all[1]["prompt"].as_str().unwrap().chars().count(), 200);

        let session = run_tool(&mut tool, ccx.clone(), Some("session")).await;
        assert_eq!(session.len(), 1);
        assert_eq!(session[0]["id"], json!("cron_session"));
        assert_eq!(session[0]["durable"], json!(false));

        let durable = run_tool(&mut tool, ccx, Some("durable")).await;
        assert_eq!(durable.len(), 1);
        assert_eq!(durable[0]["id"], json!("cron_durable"));
        assert_eq!(durable[0]["durable"], json!(true));
    }
}
