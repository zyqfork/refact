use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex as AMutex;

use crate::at_commands::at_commands::AtCommandsContext;
use crate::at_commands::at_web_search::execute_web_search;
use crate::tools::tools_description::{Tool, ToolDesc, ToolSource, ToolSourceType, json_schema_from_params};
use crate::call_validation::{ChatMessage, ChatContent, ContextEnum};

pub struct ToolWebSearch {
    pub config_path: String,
}

const DEFAULT_NUM_RESULTS: usize = 8;

#[async_trait]
impl Tool for ToolWebSearch {
    fn tool_description(&self) -> ToolDesc {
        ToolDesc {
            name: "web_search".to_string(),
            display_name: "Web Search".to_string(),
            source: ToolSource {
                source_type: ToolSourceType::Builtin,
                config_path: self.config_path.clone(),
            },
            experimental: false,
            allow_parallel: true,
            description: "Search the web and return results with titles, URLs, and snippets. Uses DuckDuckGo.".to_string(),
            input_schema: json_schema_from_params(&[("query", "string", "Search query."), ("num_results", "string", "Optional. Maximum number of results to return (default: 8).")], &["query"]),
            output_schema: None,
            annotations: None,
        }
    }

    async fn tool_execute(
        &mut self,
        _ccx: Arc<AMutex<AtCommandsContext>>,
        tool_call_id: &String,
        args: &HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>), String> {
        let query = match args.get("query") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => return Err(format!("argument `query` is not a string: {:?}", v)),
            None => return Err("Missing argument `query`".to_string()),
        };

        let num_results = args
            .get("num_results")
            .and_then(|v| match v {
                Value::String(s) => s.parse::<usize>().ok(),
                Value::Number(n) => n.as_u64().map(|n| n as usize),
                _ => None,
            })
            .unwrap_or(DEFAULT_NUM_RESULTS);

        let text = execute_web_search(&query, num_results).await?;

        let result = vec![ContextEnum::ChatMessage(ChatMessage {
            role: "tool".to_string(),
            content: ChatContent::SimpleText(text),
            tool_calls: None,
            tool_call_id: tool_call_id.clone(),
            ..Default::default()
        })];

        Ok((false, result))
    }

    fn tool_depends_on(&self) -> Vec<String> {
        vec![]
    }
}
