use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::buddy::events::BuddyEvent;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType};

const VALID_FLOWS: &[&str] = &[
    "setup",
    "setup_mcp",
    "setup_skills",
    "setup_commands",
    "setup_agents_md",
    "setup_subagents",
    "configurator",
];

pub struct ToolBuddyOpenSetupFlow {
    pub config_path: String,
}

#[async_trait]
impl Tool for ToolBuddyOpenSetupFlow {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "buddy_open_setup_flow".to_string(),
            display_name: "Buddy Open Setup Flow".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: false,
            description: "Launch a setup flow as a Buddy-owned conversation. The frontend will open the specified setup mode as a new chat.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "flow": {
                        "type": "string",
                        "description": "Which setup flow to launch.",
                        "enum": ["setup", "setup_mcp", "setup_skills", "setup_commands", "setup_agents_md", "setup_subagents", "configurator"]
                    }
                },
                "required": ["flow"]
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
        let flow = args
            .get("flow")
            .and_then(|v| v.as_str())
            .ok_or("argument `flow` is missing or not a string")?
            .to_string();

        if !VALID_FLOWS.contains(&flow.as_str()) {
            return Err(format!(
                "invalid flow '{}'. Valid flows: {}",
                flow,
                VALID_FLOWS.join(", ")
            ));
        }

        let chat_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let gcx = ccx.lock().await.global_context.clone();

        let project_root = crate::files_correction::get_project_dirs(gcx.clone())
            .await
            .into_iter()
            .next();

        if let Some(root) = &project_root {
            let conv = serde_json::json!({
                "chat_id": chat_id,
                "title": format!("Setup: {}", flow),
                "mode": flow,
                "is_buddy_chat": true,
                "buddy_chat_kind": "setup",
                "created_at": created_at,
                "last_message_at": null,
                "messages": []
            });
            let path = root.join(format!(
                ".refact/buddy/chats/conversations/{}.json",
                chat_id
            ));
            crate::buddy::storage::atomic_write_json(&path, &conv)
                .await
                .map_err(|e| format!("failed to create setup conversation: {}", e))?;
        }

        let gcx_read = gcx.read().await;
        let buddy_arc = gcx_read.buddy.clone();
        drop(gcx_read);

        let lock = buddy_arc.lock().await;
        if let Some(svc) = lock.as_ref() {
            let _ = svc.events_tx.send(BuddyEvent::NavigationRequest {
                view: "setup_flow".to_string(),
                params: Some(serde_json::json!({
                    "flow": flow,
                    "chat_id": chat_id
                })),
            });
        }

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(format!(
                    "Setup flow '{}' launched. chat_id: {}",
                    flow, chat_id
                )),
                tool_calls: None,
                tool_call_id: tool_call_id.clone(),
                ..Default::default()
            })],
        ))
    }

    fn tool_depends_on(&self) -> Vec<String> {
        vec![]
    }
}
