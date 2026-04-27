use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{ChatContent, ChatMessage, ContextEnum};
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType};

pub struct ToolBuddyOpenView {
    pub config_path: String,
}

#[async_trait]
impl Tool for ToolBuddyOpenView {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "buddy_open_view".to_string(),
            display_name: "Buddy Open View".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: false,
            description: "Navigate the frontend to a specific view. The GUI will switch to the requested page.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "view": {
                        "type": "string",
                        "description": "Which view to open.",
                        "enum": ["buddy_home", "dashboard", "chat", "stats", "setup", "settings"]
                    },
                    "params": {
                        "type": "object",
                        "description": "Optional parameters for the view (e.g., {\"chat_id\": \"...\"}). Use a flat object with string values.",
                        "additionalProperties": true
                    }
                },
                "required": ["view"]
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
        let view = args
            .get("view")
            .and_then(|v| v.as_str())
            .ok_or("argument `view` is missing or not a string")?
            .to_string();

        let valid_views = [
            "buddy_home",
            "dashboard",
            "chat",
            "stats",
            "setup",
            "settings",
        ];
        if !valid_views.contains(&view.as_str()) {
            return Err(format!(
                "invalid view '{}'. Valid views: {}",
                view,
                valid_views.join(", ")
            ));
        }

        let params = args.get("params").cloned();

        let gcx = ccx.lock().await.global_context.clone();
        let buddy_arc = gcx.read().await.buddy.clone();
        let lock = buddy_arc.lock().await;
        if let Some(svc) = lock.as_ref() {
            svc.send_navigation(view.clone(), params);
        }

        Ok((
            false,
            vec![ContextEnum::ChatMessage(ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::SimpleText(format!(
                    "Navigation request sent: open '{}'",
                    view
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
