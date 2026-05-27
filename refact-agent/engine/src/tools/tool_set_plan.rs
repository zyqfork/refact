use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::chat::internal_roles::{self, EventSubkind};
use crate::tools::tools_description::{
    json_schema_from_params, Tool, ToolDesc, ToolSource, ToolSourceType,
};
use crate::yaml_configs::customization_registry::map_legacy_mode_to_id;

pub struct ToolSetPlan {
    pub config_path: String,
}

#[async_trait]
impl Tool for ToolSetPlan {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "set_plan".to_string(),
            display_name: "Set Plan".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: false,
            description: "Update the agent's current plan. The new plan replaces the previous version visible to you and persists across compression. Use this when your understanding of the task evolves.".to_string(),
            input_schema: json_schema_from_params(
                &[
                    ("content", "string", "Markdown plan body. Required."),
                    (
                        "summary",
                        "string",
                        "Short description of what changed, ≤120 chars. Optional.",
                    ),
                ],
                &["content"],
            ),
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
        let content = string_arg(args, "content")?;
        if content.trim().is_empty() {
            return Err("argument `content` must be non-empty".to_string());
        }
        let summary = optional_string_arg(args, "summary")?;
        if summary
            .as_ref()
            .is_some_and(|summary| summary.chars().count() > 120)
        {
            return Err("argument `summary` must be at most 120 chars".to_string());
        }

        let (gcx, chat_id) = {
            let cgcx = ccx.lock().await;
            (cgcx.app.gcx.clone(), cgcx.chat_id.clone())
        };
        let session_arc = {
            let sessions = gcx.chat_sessions.read().await;
            sessions.get(&chat_id).cloned()
        }
        .ok_or_else(|| format!("chat session `{chat_id}` not found"))?;

        let report = {
            let mut session = session_arc.lock().await;
            let current_mode = map_legacy_mode_to_id(&session.thread.mode).to_string();
            let report =
                crate::chat::plan_role::install_plan(&mut session, &current_mode, &content);
            session.add_message(internal_roles::event(
                EventSubkind::SystemNotice,
                "tool.set_plan",
                json!({"version": report.version, "summary": summary}),
                format!("Plan updated to v{}", report.version),
            ));
            report
        };

        let result = json!({
            "version": report.version,
            "supersedes": report.supersedes,
        });

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(result.to_string()),
                tool_call_id: tool_call_id.clone(),
                ..Default::default()
            })],
        ))
    }
}

fn string_arg(args: &HashMap<String, Value>, name: &str) -> Result<String, String> {
    match args.get(name) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(value) => Err(format!("argument `{name}` is not a string: {value:?}")),
        None => Err(format!("argument `{name}` is missing")),
    }
}

fn optional_string_arg(
    args: &HashMap<String, Value>,
    name: &str,
) -> Result<Option<String>, String> {
    match args.get(name) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(value) => Err(format!("argument `{name}` is not a string: {value:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::chat::internal_roles::{EVENT_ROLE, PLAN_ROLE};
    use crate::chat::types::{ChatEvent, ChatSession, EventEnvelope};
    use crate::tools::tools_list::get_tools_for_mode;

    const CHAT_ID: &str = "set-plan-chat";

    async fn ccx_for_session(
        mode: &str,
    ) -> (
        Arc<crate::global_context::GlobalContext>,
        Arc<AMutex<AtCommandsContext>>,
        tokio::sync::broadcast::Receiver<Arc<String>>,
    ) {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        crate::yaml_configs::project_configs_bootstrap::global_configs_try_create_all(
            &gcx.config_dir,
        )
        .await
        .unwrap();
        let mut session = ChatSession::new(CHAT_ID.to_string());
        session.thread.mode = mode.to_string();
        let rx = session.subscribe();
        gcx.chat_sessions
            .write()
            .await
            .insert(CHAT_ID.to_string(), Arc::new(AMutex::new(session)));
        (gcx.clone(), make_ccx(gcx).await, rx)
    }

    async fn make_ccx(
        gcx: Arc<crate::global_context::GlobalContext>,
    ) -> Arc<AMutex<AtCommandsContext>> {
        Arc::new(AMutex::new(
            AtCommandsContext::new_from_app(
                AppState::from_gcx(gcx).await,
                4096,
                20,
                false,
                vec![],
                CHAT_ID.to_string(),
                None,
                "model".to_string(),
                None,
                None,
            )
            .await,
        ))
    }

    fn content_text(message: &ChatMessage) -> &str {
        match &message.content {
            ChatContent::SimpleText(text) => text,
            ChatContent::Multimodal(_) | ChatContent::ContextFiles(_) => {
                panic!("expected text message")
            }
        }
    }

    fn tool_result_json(messages: &[ContextEnum]) -> Value {
        match &messages[0] {
            ContextEnum::ChatMessage(message) => {
                serde_json::from_str(content_text(message)).unwrap()
            }
            ContextEnum::ContextFile(_) => panic!("expected tool chat message"),
        }
    }

    fn event_from_json(json: Arc<String>) -> ChatEvent {
        serde_json::from_str::<EventEnvelope>(&json).unwrap().event
    }

    #[tokio::test]
    async fn happy_path() {
        let (gcx, ccx, mut rx) = ccx_for_session("agent").await;
        let mut tool = ToolSetPlan {
            config_path: String::new(),
        };
        let args = HashMap::from([
            ("content".to_string(), json!("## Plan\n- do it")),
            ("summary".to_string(), json!("new direction")),
        ]);

        let (_, messages) = tool
            .tool_execute(ccx, &"call".to_string(), &args)
            .await
            .unwrap();

        assert_eq!(
            tool_result_json(&messages),
            json!({"version": 1, "supersedes": null})
        );

        let session_arc = gcx
            .chat_sessions
            .read()
            .await
            .get(CHAT_ID)
            .cloned()
            .unwrap();
        let mut session = session_arc.lock().await;
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, PLAN_ROLE);
        assert_eq!(content_text(&session.messages[0]), "## Plan\n- do it");
        assert_eq!(session.messages[0].extra["plan"]["version"], json!(1));
        assert_eq!(session.messages[0].extra["plan"]["mode"], json!("agent"));
        assert_eq!(session.messages[1].role, EVENT_ROLE);
        assert_eq!(content_text(&session.messages[1]), "Plan updated to v1");
        assert_eq!(
            session.messages[1].extra["event"],
            json!({
                "subkind": "system_notice",
                "source": "tool.set_plan",
                "payload": {"version": 1, "summary": "new direction"}
            })
        );
        let first_plan_id = session.messages[0].message_id.clone();
        drop(session);

        match event_from_json(rx.try_recv().unwrap()) {
            ChatEvent::MessageAdded { message, index } => {
                assert_eq!(index, 0);
                assert_eq!(message.role, PLAN_ROLE);
            }
            other => panic!("expected plan MessageAdded, got {other:?}"),
        }
        match event_from_json(rx.try_recv().unwrap()) {
            ChatEvent::MessageAdded { message, index } => {
                assert_eq!(index, 1);
                assert_eq!(message.role, EVENT_ROLE);
            }
            other => panic!("expected event MessageAdded, got {other:?}"),
        }

        let mut tool = ToolSetPlan {
            config_path: String::new(),
        };
        let args = HashMap::from([("content".to_string(), json!("second"))]);
        tool.tool_execute(make_ccx(gcx.clone()).await, &"call2".to_string(), &args)
            .await
            .unwrap();
        session = session_arc.lock().await;
        assert_eq!(session.messages[2].extra["plan"]["version"], json!(2));
        assert_eq!(
            session.messages[2].extra["plan"]["supersedes"],
            json!(first_plan_id)
        );
    }

    #[tokio::test]
    async fn empty_content_rejected() {
        let (_gcx, ccx, _rx) = ccx_for_session("agent").await;
        let mut tool = ToolSetPlan {
            config_path: String::new(),
        };
        let args = HashMap::from([("content".to_string(), json!("  \n\t"))]);

        let err = tool
            .tool_execute(ccx, &"call".to_string(), &args)
            .await
            .unwrap_err();

        assert_eq!(err, "argument `content` must be non-empty");
    }

    #[tokio::test]
    async fn not_available_in_no_tools_mode() {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        crate::yaml_configs::project_configs_bootstrap::global_configs_try_create_all(
            &gcx.config_dir,
        )
        .await
        .unwrap();

        let supported = get_tools_for_mode(gcx.clone(), "agent", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");
        let task_planner = get_tools_for_mode(gcx.clone(), "task_planner", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");
        let task_agent = get_tools_for_mode(gcx.clone(), "task_agent", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");
        let no_tools = get_tools_for_mode(gcx.clone(), "NO_TOOLS", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");
        let shell = get_tools_for_mode(gcx.clone(), "shell", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");
        let explore = get_tools_for_mode(gcx, "explore", None)
            .await
            .into_iter()
            .any(|tool| tool.tool_description().name == "set_plan");

        assert!(supported);
        assert!(task_planner);
        assert!(task_agent);
        assert!(!no_tools);
        assert!(!shell);
        assert!(!explore);
    }
}
