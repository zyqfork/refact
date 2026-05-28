use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::exec::types::normalize_workspace_path;
use crate::exec::{ExecMode, ExecProcessFilter, ExecProcessSnapshot, ExecStatusKind};
use crate::files_correction::get_active_project_path;
use crate::postprocessing::pp_command_output::OutputFilter;
use crate::tools::file_edit::auxiliary::active_execution_scope;
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType};
use crate::worktrees::scope::ExecutionScope;

pub struct ToolCleanBackgroundProcesses {
    pub config_path: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CleanScope {
    Chat,
    Owner,
    Workspace,
    All,
}

#[async_trait]
impl Tool for ToolCleanBackgroundProcesses {
    async fn tool_execute(
        &mut self,
        ccx: Arc<AMutex<AtCommandsContext>>,
        tool_call_id: &String,
        args: &HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>), String> {
        let scope = parse_scope(args)?;
        let include_services = parse_include_services(args)?;
        let (gcx, exec_registry, execution_scope, chat_id) = {
            let ccx = ccx.lock().await;
            (
                ccx.app.gcx.clone(),
                ccx.app.runtime.exec_registry.clone(),
                ccx.execution_scope.clone(),
                ccx.chat_id.clone(),
            )
        };
        let workspace = if scope == CleanScope::Workspace {
            Some(current_workspace(gcx, execution_scope.as_ref()).await?)
        } else {
            None
        };
        let base_filter = scoped_filter(scope, &chat_id, tool_call_id, workspace);
        let mut killed = Vec::new();
        for mode in target_modes(include_services) {
            for status in [ExecStatusKind::Starting, ExecStatusKind::Running] {
                let mut filter = base_filter.clone();
                filter.mode = Some(mode.clone());
                filter.status = Some(status);
                killed.extend(exec_registry.remove_by_owner(filter).await?);
            }
        }
        killed.sort_by(|a, b| a.meta.process_id.as_str().cmp(b.meta.process_id.as_str()));
        let body = json!({
            "killed_count": killed.len(),
            "killed": killed.iter().map(killed_value).collect::<Vec<_>>(),
        });
        let mut extra = serde_json::Map::new();
        extra.insert("clean_background_processes".to_string(), body.clone());

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(body.to_string()),
                tool_calls: None,
                tool_call_id: tool_call_id.clone(),
                tool_failed: Some(false),
                output_filter: Some(OutputFilter::no_limits()),
                extra,
                ..Default::default()
            })],
        ))
    }

    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "clean_background_processes".to_string(),
            display_name: "Clean Background Processes".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: false,
            description: "Kill and reap all non-terminal background processes. Use to clean up after experiments. Services are excluded by default unless include_services=true.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["chat", "owner", "workspace", "all"],
                        "default": "chat",
                        "description": "Which set of processes to target."
                    },
                    "include_services": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also kill Service-mode processes."
                    }
                }
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

fn parse_scope(args: &HashMap<String, Value>) -> Result<CleanScope, String> {
    match args.get("scope") {
        Some(Value::String(scope)) if scope.trim().is_empty() => Ok(CleanScope::Chat),
        Some(Value::String(scope)) => match scope.trim() {
            "chat" => Ok(CleanScope::Chat),
            "owner" => Ok(CleanScope::Owner),
            "workspace" => Ok(CleanScope::Workspace),
            "all" => Ok(CleanScope::All),
            other => Err(format!(
                "Invalid scope `{other}`. Must be one of: chat, owner, workspace, all"
            )),
        },
        Some(value) => Err(format!("argument `scope` is not a string: {value:?}")),
        None => Ok(CleanScope::Chat),
    }
}

fn parse_include_services(args: &HashMap<String, Value>) -> Result<bool, String> {
    match args.get("include_services") {
        Some(Value::Bool(value)) => Ok(*value),
        Some(value) => Err(format!(
            "argument `include_services` is not a boolean: {value:?}"
        )),
        None => Ok(false),
    }
}

async fn current_workspace(
    gcx: Arc<crate::global_context::GlobalContext>,
    execution_scope: Option<&ExecutionScope>,
) -> Result<PathBuf, String> {
    if let Some(scope) = active_execution_scope(execution_scope) {
        return Ok(normalize_workspace_path(scope.effective_root()));
    }
    get_active_project_path(gcx)
        .await
        .map(|path| normalize_workspace_path(&path))
        .ok_or_else(|| "No active project for background process cleanup".to_string())
}

fn scoped_filter(
    scope: CleanScope,
    chat_id: &str,
    tool_call_id: &str,
    workspace: Option<PathBuf>,
) -> ExecProcessFilter {
    match scope {
        CleanScope::Chat => ExecProcessFilter {
            chat_id: Some(chat_id.to_string()),
            ..ExecProcessFilter::default()
        },
        CleanScope::Owner => ExecProcessFilter {
            tool_call_id: Some(tool_call_id.to_string()),
            ..ExecProcessFilter::default()
        },
        CleanScope::Workspace => ExecProcessFilter {
            workspace,
            ..ExecProcessFilter::default()
        },
        CleanScope::All => ExecProcessFilter::default(),
    }
}

fn target_modes(include_services: bool) -> Vec<ExecMode> {
    if include_services {
        vec![ExecMode::Background, ExecMode::Service]
    } else {
        vec![ExecMode::Background]
    }
}

fn killed_value(snapshot: &ExecProcessSnapshot) -> Value {
    json!({
        "process_id": snapshot.meta.process_id.as_str(),
        "short_description": snapshot.meta.short_description.as_str(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::exec::types::DEFAULT_EXEC_OUTPUT_LIMIT_BYTES;
    use crate::exec::{ExecOwnerMeta, ExecProcessId, ExecProcessMeta};

    async fn test_ccx(
        chat_id: &str,
    ) -> (
        Arc<crate::global_context::GlobalContext>,
        Arc<AMutex<AtCommandsContext>>,
    ) {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        let ccx = AtCommandsContext::new_with_abort(
            AppState::from_gcx(gcx.clone()).await,
            4096,
            20,
            false,
            Vec::new(),
            chat_id.to_string(),
            None,
            "model".to_string(),
            None,
            None,
            None,
        )
        .await;
        (gcx, Arc::new(AMutex::new(ccx)))
    }

    async fn register_running(
        gcx: &crate::global_context::GlobalContext,
        process_id: &str,
        mode: ExecMode,
        chat_id: &str,
        short_description: &str,
    ) -> ExecProcessId {
        let snapshot = gcx
            .exec_registry
            .register(
                ExecProcessMeta::new(mode, "test command".to_string())
                    .with_process_id(ExecProcessId(process_id.to_string()))
                    .with_owner(ExecOwnerMeta {
                        chat_id: Some(chat_id.to_string()),
                        tool_call_id: Some("owner-call".to_string()),
                        ..ExecOwnerMeta::default()
                    })
                    .with_short_description(short_description.to_string()),
                DEFAULT_EXEC_OUTPUT_LIMIT_BYTES,
            )
            .await;
        gcx.exec_registry
            .mark_started(&snapshot.meta.process_id)
            .await
            .unwrap();
        snapshot.meta.process_id
    }

    async fn run_tool(
        ccx: Arc<AMutex<AtCommandsContext>>,
        args: HashMap<String, Value>,
    ) -> Result<ChatMessage, String> {
        let mut tool = ToolCleanBackgroundProcesses {
            config_path: String::new(),
        };
        let (_, messages) = tool
            .tool_execute(ccx, &"cleanup-call".to_string(), &args)
            .await?;
        match messages.into_iter().next().unwrap() {
            ContextEnum::ChatMessage(message) => Ok(message),
            ContextEnum::ContextFile(_) => panic!("expected chat message"),
        }
    }

    fn args(entries: Vec<(&str, Value)>) -> HashMap<String, Value> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    fn body(message: &ChatMessage) -> Value {
        match &message.content {
            ChatContent::SimpleText(text) => serde_json::from_str(text).unwrap(),
            _ => panic!("expected text body"),
        }
    }

    fn killed_ids(body: &Value) -> Vec<String> {
        body["killed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["process_id"].as_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn chat_scope_kills_only_this_chat() {
        let (gcx, ccx) = test_ccx("chat-a").await;
        let killed = register_running(
            &gcx,
            "exec_chat_a_background",
            ExecMode::Background,
            "chat-a",
            "chat a process",
        )
        .await;
        let kept = register_running(
            &gcx,
            "exec_chat_b_background",
            ExecMode::Background,
            "chat-b",
            "chat b process",
        )
        .await;

        let message = run_tool(ccx, HashMap::new()).await.unwrap();
        let body = body(&message);

        assert_eq!(body["killed_count"], json!(1));
        assert_eq!(killed_ids(&body), vec![killed.as_str().to_string()]);
        assert!(gcx.exec_registry.get(&killed).await.is_none());
        assert!(gcx.exec_registry.get(&kept).await.is_some());
    }

    #[tokio::test]
    async fn services_excluded_by_default() {
        let (gcx, ccx) = test_ccx("chat").await;
        let background = register_running(
            &gcx,
            "exec_background_default",
            ExecMode::Background,
            "chat",
            "background process",
        )
        .await;
        let service = register_running(
            &gcx,
            "exec_service_default",
            ExecMode::Service,
            "chat",
            "service process",
        )
        .await;

        let message = run_tool(ccx, HashMap::new()).await.unwrap();
        let body = body(&message);

        assert_eq!(body["killed_count"], json!(1));
        assert_eq!(killed_ids(&body), vec![background.as_str().to_string()]);
        assert!(gcx.exec_registry.get(&background).await.is_none());
        assert!(gcx.exec_registry.get(&service).await.is_some());
    }

    #[tokio::test]
    async fn include_services_true_kills_them() {
        let (gcx, ccx) = test_ccx("chat").await;
        let background = register_running(
            &gcx,
            "exec_background_included",
            ExecMode::Background,
            "chat",
            "background process",
        )
        .await;
        let service = register_running(
            &gcx,
            "exec_service_included",
            ExecMode::Service,
            "chat",
            "service process",
        )
        .await;

        let message = run_tool(ccx, args(vec![("include_services", json!(true))]))
            .await
            .unwrap();
        let body = body(&message);

        assert_eq!(body["killed_count"], json!(2));
        assert_eq!(
            killed_ids(&body),
            vec![
                background.as_str().to_string(),
                service.as_str().to_string()
            ]
        );
        assert!(gcx.exec_registry.get(&background).await.is_none());
        assert!(gcx.exec_registry.get(&service).await.is_none());
    }

    #[tokio::test]
    async fn foreground_unaffected() {
        let (gcx, ccx) = test_ccx("chat").await;
        let background = register_running(
            &gcx,
            "exec_background_foreground_test",
            ExecMode::Background,
            "chat",
            "background process",
        )
        .await;
        let foreground = register_running(
            &gcx,
            "exec_foreground_unaffected",
            ExecMode::Foreground,
            "chat",
            "foreground process",
        )
        .await;

        let message = run_tool(ccx, args(vec![("include_services", json!(true))]))
            .await
            .unwrap();
        let body = body(&message);

        assert_eq!(body["killed_count"], json!(1));
        assert_eq!(killed_ids(&body), vec![background.as_str().to_string()]);
        assert!(gcx.exec_registry.get(&background).await.is_none());
        assert!(gcx.exec_registry.get(&foreground).await.is_some());
    }
}
