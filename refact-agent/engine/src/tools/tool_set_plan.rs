use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::chat::internal_roles::{self, EventSubkind};
use crate::chat::plan_role::PlanInstallReport;
use crate::chat::types::ChatSession;
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
            let report = queue_plan_side_effect(&mut session, &current_mode, &content);
            session.queue_post_tool_side_effect(internal_roles::event(
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

fn queue_plan_side_effect(session: &mut ChatSession, mode: &str, body: &str) -> PlanInstallReport {
    let previous = current_plan_including_queued(session);
    let version = previous
        .and_then(plan_version)
        .map_or(1, |version| version + 1);
    let supersedes = previous.map(|message| message.message_id.clone());
    session.queue_post_tool_side_effect(internal_roles::plan(
        mode,
        version,
        body,
        supersedes.clone(),
    ));
    PlanInstallReport {
        version,
        supersedes,
    }
}

fn current_plan_including_queued(session: &ChatSession) -> Option<&ChatMessage> {
    session
        .messages
        .iter()
        .chain(session.post_tool_side_effects.iter())
        .enumerate()
        .filter_map(|(index, message)| {
            plan_version(message).map(|version| (index, version, message))
        })
        .max_by_key(|(index, version, _)| (*version, *index))
        .map(|(_, _, message)| message)
}

fn plan_version(message: &ChatMessage) -> Option<u32> {
    if message.role != internal_roles::PLAN_ROLE {
        return None;
    }
    message
        .extra
        .get("plan")?
        .get("version")?
        .as_u64()
        .and_then(|version| u32::try_from(version).ok())
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
    use crate::call_validation::{ChatToolCall, ChatToolFunction};
    use crate::chat::internal_roles::{EVENT_ROLE, PLAN_ROLE};
    use crate::chat::types::{ChatEvent, ChatSession, EventEnvelope};
    use crate::llm::adapter::{AdapterSettings, LlmWireAdapter};
    use crate::llm::adapters::openai_chat::OpenAiChatAdapter;
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

    fn assistant_tool_call(tool_call_id: &str, name: &str, arguments: &str) -> ChatMessage {
        ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::SimpleText(String::new()),
            tool_calls: Some(vec![ChatToolCall {
                id: tool_call_id.to_string(),
                index: Some(0),
                function: ChatToolFunction {
                    name: name.to_string(),
                    arguments: arguments.to_string(),
                },
                tool_type: "function".to_string(),
                extra_content: None,
            }]),
            ..Default::default()
        }
    }

    fn default_settings() -> AdapterSettings {
        AdapterSettings {
            api_key: "test-key".to_string(),
            auth_token: String::new(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            extra_headers: Default::default(),
            model_name: "gpt-4.1".to_string(),
            supports_tools: true,
            supports_reasoning: true,
            reasoning_type: None,
            supports_temperature: true,
            supports_max_completion_tokens: false,
            eof_is_done: false,
            supports_web_search: false,
            supports_cache_control: false,
        }
    }

    fn assert_openai_tool_result_not_preceded_by_hidden_role(messages: Vec<ChatMessage>) {
        let req = crate::llm::canonical::LlmRequest::new("gpt-4.1".to_string(), messages);
        let body = OpenAiChatAdapter
            .build_http(&req, &default_settings())
            .unwrap()
            .body;
        let wire_messages = body["messages"].as_array().unwrap();
        let tool_index = wire_messages
            .iter()
            .position(|message| message["role"] == "tool")
            .expect("tool result missing from wire messages");
        let prior = &wire_messages[tool_index - 1];
        assert_eq!(prior["role"], "assistant");
        assert!(
            prior.get("tool_calls").is_some(),
            "prior message: {prior:?}"
        );
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
        assert!(session.messages.is_empty());
        assert_eq!(session.post_tool_side_effects.len(), 2);
        assert_eq!(session.post_tool_side_effects[0].role, PLAN_ROLE);
        assert_eq!(
            content_text(&session.post_tool_side_effects[0]),
            "## Plan\n- do it"
        );
        assert_eq!(
            session.post_tool_side_effects[0].extra["plan"]["version"],
            json!(1)
        );
        assert_eq!(
            session.post_tool_side_effects[0].extra["plan"]["mode"],
            json!("agent")
        );
        assert_eq!(session.post_tool_side_effects[1].role, EVENT_ROLE);
        assert_eq!(
            content_text(&session.post_tool_side_effects[1]),
            "Plan updated to v1"
        );
        assert_eq!(
            session.post_tool_side_effects[1].extra["event"],
            json!({
                "subkind": "system_notice",
                "source": "tool.set_plan",
                "payload": {"version": 1, "summary": "new direction"}
            })
        );
        let first_plan_id = session.post_tool_side_effects[0].message_id.clone();
        session.drain_post_tool_side_effects();

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
        drop(session);

        let mut tool = ToolSetPlan {
            config_path: String::new(),
        };
        let args = HashMap::from([("content".to_string(), json!("second"))]);
        tool.tool_execute(make_ccx(gcx.clone()).await, &"call2".to_string(), &args)
            .await
            .unwrap();
        session = session_arc.lock().await;
        assert_eq!(
            session.post_tool_side_effects[0].extra["plan"]["version"],
            json!(2)
        );
        assert_eq!(
            session.post_tool_side_effects[0].extra["plan"]["supersedes"],
            json!(first_plan_id)
        );
    }

    #[tokio::test]
    async fn set_plan_side_effects_are_after_tool_result() {
        let (gcx, ccx, _rx) = ccx_for_session("agent").await;
        let session_arc = gcx
            .chat_sessions
            .read()
            .await
            .get(CHAT_ID)
            .cloned()
            .unwrap();
        session_arc.lock().await.add_message(assistant_tool_call(
            "call-plan",
            "set_plan",
            r#"{"content":"new"}"#,
        ));

        let mut tool = ToolSetPlan {
            config_path: String::new(),
        };
        let args = HashMap::from([("content".to_string(), json!("new"))]);
        let (_, results) = tool
            .tool_execute(ccx, &"call-plan".to_string(), &args)
            .await
            .unwrap();

        let mut session = session_arc.lock().await;
        for message in results {
            let ContextEnum::ChatMessage(message) = message else {
                panic!("expected chat message")
            };
            session.add_message(message);
        }
        session.drain_post_tool_side_effects();

        let roles: Vec<_> = session
            .messages
            .iter()
            .map(|message| message.role.as_str())
            .collect();
        assert_eq!(roles, vec!["assistant", "tool", "plan", "event"]);
        assert_openai_tool_result_not_preceded_by_hidden_role(session.messages.clone());
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
