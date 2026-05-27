use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::scheduler::{CronStore, JsonFileCronStore};
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType};

pub struct ToolCronDelete {
    pub config_path: String,
    #[cfg(test)]
    store: Option<Arc<dyn CronStore>>,
    #[cfg(test)]
    runner_change: Option<Arc<tokio::sync::Notify>>,
}

impl ToolCronDelete {
    pub fn new(config_path: String) -> Self {
        Self {
            config_path,
            #[cfg(test)]
            store: None,
            #[cfg(test)]
            runner_change: None,
        }
    }

    #[cfg(test)]
    fn with_store(
        config_path: String,
        store: Arc<dyn CronStore>,
        runner_change: Arc<tokio::sync::Notify>,
    ) -> Self {
        Self {
            config_path,
            store: Some(store),
            runner_change: Some(runner_change),
        }
    }

    async fn store(
        &self,
        ccx: Arc<AMutex<AtCommandsContext>>,
    ) -> Result<Arc<dyn CronStore>, String> {
        #[cfg(test)]
        if let Some(store) = &self.store {
            return Ok(store.clone());
        }

        let gcx = ccx.lock().await.global_context.clone();
        let project_root = crate::files_correction::get_active_workspace_folder(gcx.clone())
            .await
            .unwrap_or_else(|| gcx.config_dir.clone());
        Ok(Arc::new(JsonFileCronStore::new(project_root)?))
    }

    fn notify_runner_change(&self) {
        #[cfg(test)]
        if let Some(notify) = &self.runner_change {
            notify.notify_waiters();
            return;
        }

        crate::scheduler::runner::notify_runner_change();
    }
}

#[async_trait]
impl Tool for ToolCronDelete {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "cron_delete".to_string(),
            display_name: "Delete Scheduled Task".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: false,
            description: "Cancel a scheduled task by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"]
            }),
            output_schema: None,
            annotations: None,
        }
    }

    async fn tool_execute(
        &mut self,
        ccx: Arc<AMutex<AtCommandsContext>>,
        tool_call_id: &String,
        args: &HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>), String> {
        let id = match args.get("id") {
            Some(Value::String(id)) => id.clone(),
            Some(value) => return Err(format!("argument `id` is not a string: {value:?}")),
            None => return Err("argument `id` is missing".to_string()),
        };

        let removed = self.store(ccx).await?.remove(&id).await?;
        self.notify_runner_change();

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(json!({ "removed": removed }).to_string()),
                tool_call_id: tool_call_id.clone(),
                tool_failed: Some(false),
                ..Default::default()
            })],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::scheduler::{InMemoryCronStore, ScheduledTask};

    fn args(entries: Vec<(&str, Value)>) -> HashMap<String, Value> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    fn test_task(id: &str) -> ScheduledTask {
        let mut task = ScheduledTask::new(
            "*/5 * * * *".to_string(),
            "Check the build".to_string(),
            "Check build".to_string(),
            true,
            false,
            1,
        );
        task.id = id.to_string();
        task
    }

    async fn test_ccx() -> Arc<AMutex<AtCommandsContext>> {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        let ccx = AtCommandsContext::new_with_abort(
            AppState::from_gcx(gcx).await,
            4096,
            20,
            false,
            Vec::new(),
            "cron-delete-test".to_string(),
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
        tool: &mut ToolCronDelete,
        ccx: Arc<AMutex<AtCommandsContext>>,
        args: HashMap<String, Value>,
    ) -> Value {
        let (_, contexts) = tool
            .tool_execute(ccx, &"tool_call".to_string(), &args)
            .await
            .unwrap();
        let ContextEnum::ChatMessage(message) = contexts.into_iter().next().unwrap() else {
            panic!("expected chat message")
        };
        let ChatContent::SimpleText(text) = message.content else {
            panic!("expected simple text")
        };
        serde_json::from_str(&text).unwrap()
    }

    #[tokio::test]
    async fn removes_existing() {
        let store = Arc::new(InMemoryCronStore::new());
        store.add(test_task("cron_1")).await.unwrap();
        let notify = Arc::new(tokio::sync::Notify::new());
        let notified = notify.notified();
        let ccx = test_ccx().await;
        let mut tool = ToolCronDelete::with_store(String::new(), store.clone(), notify.clone());

        let result = run_tool(&mut tool, ccx, args(vec![("id", json!("cron_1"))])).await;

        assert_eq!(result, json!({ "removed": true }));
        assert!(store.list().await.is_empty());
        tokio::time::timeout(std::time::Duration::from_secs(1), notified)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn missing_id_returns_false() {
        let store = Arc::new(InMemoryCronStore::new());
        let notify = Arc::new(tokio::sync::Notify::new());
        let notified = notify.notified();
        let ccx = test_ccx().await;
        let mut tool = ToolCronDelete::with_store(String::new(), store, notify.clone());

        let result = run_tool(&mut tool, ccx, args(vec![("id", json!("cron_missing"))])).await;

        assert_eq!(result, json!({ "removed": false }));
        tokio::time::timeout(std::time::Duration::from_secs(1), notified)
            .await
            .unwrap();
    }
}
