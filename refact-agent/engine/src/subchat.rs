use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::collections::HashSet;
use tokio::sync::{broadcast, Mutex as AMutex, RwLock as ARwLock};
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

use crate::caps::resolve_chat_model;
use crate::tools::tools_description::ToolDesc;
use crate::tools::tools_list::get_available_tools;
use crate::at_commands::at_commands::AtCommandsContext;
use crate::call_validation::{
    ChatContent, ChatMeta, ChatMode, ChatToolCall, SamplingParameters, ChatMessage, ChatUsage,
    ReasoningEffort,
};
use crate::global_context::{GlobalContext, try_load_caps_quickly_if_not_present};
use crate::scratchpad_abstract::HasTokenizerAndEot;
use crate::chat::prepare::{prepare_chat_passthrough, ChatPrepareOptions};
use crate::chat::stream_core::{
    run_llm_stream, StreamRunParams, NoopCollector, ChoiceFinal, normalize_tool_call,
};
use crate::chat::tools::{execute_tools, ExecuteToolsOptions};
use crate::chat::types::{ThreadParams, ChatCommand, CommandRequest, SessionState, ChatEvent};
use crate::chat::{get_or_create_session_with_trajectory, process_command_queue, maybe_save_trajectory};

const MAX_NEW_TOKENS: usize = 4096;

#[derive(Clone, Default)]
pub struct SubchatConfig {
    pub tools: Option<Vec<String>>,
    pub temperature: Option<f32>,
    pub max_new_tokens: Option<usize>,
    pub n_ctx: Option<usize>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub prepend_system_prompt: bool,
    pub max_steps: usize,
    pub save_trajectory: bool,
    pub chat_id: Option<String>,
    pub title: Option<String>,
    pub parent_id: Option<String>,
    pub link_type: Option<String>,
    pub mode: String,
}

impl SubchatConfig {
    #[allow(dead_code)]
    pub fn stateless() -> Self {
        Self {
            max_steps: 10,
            mode: "AGENT".to_string(),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn stateful(parent_id: Option<String>) -> Self {
        Self {
            max_steps: 10,
            mode: "TASK_AGENT".to_string(),
            save_trajectory: true,
            parent_id,
            link_type: Some("subagent".to_string()),
            ..Default::default()
        }
    }
}

pub struct SubchatResult {
    pub messages: Vec<ChatMessage>,
    pub usage: ChatUsage,
    #[allow(dead_code)]
    pub chat_id: Option<String>,
}

fn has_final_answer(messages: &[ChatMessage]) -> bool {
    messages.iter().rev()
        .find(|m| m.role == "assistant")
        .map(|m| m.tool_calls.as_ref().map_or(true, |tc| tc.is_empty()))
        .unwrap_or(false)
}

pub async fn run_subchat(
    gcx: Arc<ARwLock<GlobalContext>>,
    model: &str,
    messages: Vec<ChatMessage>,
    config: SubchatConfig,
) -> Result<SubchatResult, String> {
    if config.save_trajectory {
        run_subchat_stateful(gcx, model, messages, config).await
    } else {
        run_subchat_stateless(gcx, model, messages, config).await
    }
}

async fn run_subchat_stateless(
    gcx: Arc<ARwLock<GlobalContext>>,
    model: &str,
    messages: Vec<ChatMessage>,
    config: SubchatConfig,
) -> Result<SubchatResult, String> {
    let n_ctx = config.n_ctx.unwrap_or(32000);
    let ccx = Arc::new(AMutex::new(
        AtCommandsContext::new(
            gcx.clone(),
            n_ctx,
            1,
            false,
            messages.clone(),
            "subchat-stateless".to_string(),
            false,
            model.to_string(),
            None,
            None,
        ).await
    ));

    let mut usage = ChatUsage::default();
    let mut current_messages = messages;

    for step in 0..config.max_steps {
        let results = subchat_single(
            ccx.clone(),
            model,
            current_messages.clone(),
            config.tools.clone(),
            None,
            false,
            config.temperature,
            config.max_new_tokens,
            1,
            config.reasoning_effort.clone(),
            config.prepend_system_prompt && step == 0,
            Some(&mut usage),
            None,
            None,
        ).await?;

        current_messages = results.into_iter().next().unwrap_or(current_messages);

        if has_final_answer(&current_messages) {
            break;
        }
    }

    Ok(SubchatResult {
        messages: current_messages,
        usage,
        chat_id: None,
    })
}

async fn run_subchat_stateful(
    gcx: Arc<ARwLock<GlobalContext>>,
    model: &str,
    messages: Vec<ChatMessage>,
    config: SubchatConfig,
) -> Result<SubchatResult, String> {
    let chat_id = config.chat_id.clone().unwrap_or_else(|| format!("subchat-{}", Uuid::new_v4()));
    let title = config.title.clone().unwrap_or_else(|| "Subchat".to_string());

    let sessions = {
        let gcx_locked = gcx.read().await;
        gcx_locked.chat_sessions.clone()
    };

    let session_arc = get_or_create_session_with_trajectory(gcx.clone(), &sessions, &chat_id).await;

    {
        let mut session = session_arc.lock().await;

        session.thread = ThreadParams {
            id: chat_id.clone(),
            title: title.clone(),
            model: model.to_string(),
            mode: config.mode.clone(),
            tool_use: config.tools.as_ref().map(|t| t.join(",")).unwrap_or_else(|| "agent".to_string()),
            boost_reasoning: false,
            context_tokens_cap: config.n_ctx,
            include_project_info: true,
            checkpoints_enabled: false,
            is_title_generated: true,
            automatic_patch: false,
            task_meta: None,
            parent_id: config.parent_id.clone(),
            link_type: config.link_type.clone(),
        };

        if session.messages.is_empty() {
            for msg in messages {
                session.add_message(msg);
            }
        }

        session.increment_version();
    }

    maybe_save_trajectory(gcx.clone(), session_arc.clone()).await;

    let mut event_rx = {
        let session = session_arc.lock().await;
        session.event_tx.subscribe()
    };

    {
        let mut session = session_arc.lock().await;

        let request = CommandRequest {
            client_request_id: Uuid::new_v4().to_string(),
            priority: false,
            command: ChatCommand::Regenerate {},
        };
        session.command_queue.push_back(request);
        session.touch();

        let processor_running = session.queue_processor_running.clone();
        let queue_notify = session.queue_notify.clone();

        drop(session);

        if !processor_running.swap(true, Ordering::SeqCst) {
            tokio::spawn(process_command_queue(gcx.clone(), session_arc.clone(), processor_running));
        } else {
            queue_notify.notify_one();
        }
    }

    info!("Started stateful subchat {} (model: {}), waiting for completion...", chat_id, model);

    let timeout = tokio::time::Duration::from_secs(60 * 30);
    let start = tokio::time::Instant::now();
    let mut saw_work = false;
    let mut tool_phases = 0usize;
    let mut prev_state = SessionState::Idle;

    loop {
        if start.elapsed() > timeout {
            return Err(format!("Subchat {} timed out after 30 minutes", chat_id));
        }

        match event_rx.recv().await {
            Ok(envelope) => {
                match envelope.event {
                    ChatEvent::StreamStarted { .. } | ChatEvent::StreamDelta { .. } | ChatEvent::StreamFinished { .. } => {
                        saw_work = true;
                    }
                    ChatEvent::RuntimeUpdated { state, queue_size, error, .. } => {
                        if state == SessionState::ExecutingTools && prev_state != SessionState::ExecutingTools {
                            tool_phases += 1;
                            if tool_phases > config.max_steps {
                                return Err(format!("Subchat {} exceeded max_steps ({})", chat_id, config.max_steps));
                            }
                        }
                        prev_state = state;
                        if state != SessionState::Idle || queue_size > 0 {
                            saw_work = true;
                        }
                        match state {
                            SessionState::Idle if queue_size == 0 && saw_work => {
                                let session = session_arc.lock().await;
                                if has_final_answer(&session.messages) {
                                    info!("Subchat {} completed", chat_id);
                                    break;
                                }
                            }
                            SessionState::Paused => {
                                return Err(format!(
                                    "Subchat {} requires tool confirmation. Use TASK_AGENT mode.",
                                    chat_id
                                ));
                            }
                            SessionState::WaitingIde => {
                                return Err(format!(
                                    "Subchat {} requires IDE interaction which is not supported.",
                                    chat_id
                                ));
                            }
                            SessionState::Error => {
                                let err_msg = error.unwrap_or_else(|| "Unknown error".to_string());
                                return Err(format!("Subchat {} error: {}", chat_id, err_msg));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                saw_work = true;
                let session = session_arc.lock().await;
                let state = session.runtime.state;
                let queue_size = session.command_queue.len();
                if state == SessionState::Idle && queue_size == 0 && has_final_answer(&session.messages) {
                    info!("Subchat {} completed (detected after lag)", chat_id);
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                return Err(format!("Subchat {} event channel closed unexpectedly", chat_id));
            }
        }
    }

    let (result_messages, usage) = {
        let session = session_arc.lock().await;
        let total_usage = session.messages.iter()
            .filter_map(|m| m.usage.as_ref())
            .fold(ChatUsage::default(), |mut acc, u| {
                acc.prompt_tokens += u.prompt_tokens;
                acc.completion_tokens += u.completion_tokens;
                acc.total_tokens += u.total_tokens;
                acc
            });
        (session.messages.clone(), total_usage)
    };

    Ok(SubchatResult {
        messages: result_messages,
        usage,
        chat_id: Some(chat_id),
    })
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    let s = s.trim().replace('\n', " ");
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

fn extract_paths_from_tool_args(tool_name: &str, args_json: &str) -> Vec<String> {
    let v: Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let keys: &[&str] = match tool_name {
        "cat" => &["paths"],
        "tree" => &["path"],
        "search_semantic" | "search_pattern" => &["scope"],
        "create_textdoc" | "update_textdoc" | "update_textdoc_regex" | "update_textdoc_by_lines" => &["path"],
        "mv" => &["source", "destination"],
        "rm" => &["path"],
        _ => &[],
    };

    let mut out = Vec::new();
    for k in keys {
        if let Some(val) = v.get(*k) {
            if let Some(s) = val.as_str() {
                if *k == "paths" {
                    for part in s.split(',') {
                        let p = part.trim().split(':').next().unwrap_or("").trim();
                        if !p.is_empty() && p != "workspace" {
                            out.push(p.to_string());
                        }
                    }
                } else if s != "workspace" && !s.is_empty() {
                    out.push(s.to_string());
                }
            }
        }
    }

    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

async fn execute_pending_tool_calls(
    ccx: Arc<AMutex<AtCommandsContext>>,
    model_id: &str,
    mut messages: Vec<ChatMessage>,
    tools_subset: &[String],
    tx_toolid_mb: Option<String>,
    tx_chatid_mb: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    let gcx = ccx.lock().await.global_context.clone();
    let last = match messages.last() {
        Some(m) => m,
        None => return Ok(messages),
    };
    let tool_calls = match &last.tool_calls {
        Some(tc) if !tc.is_empty() => tc.clone(),
        _ => return Ok(messages),
    };

    let mut allowed: Vec<ChatToolCall> = vec![];
    let mut denied_msgs: Vec<ChatMessage> = vec![];

    for tc in tool_calls.iter() {
        if !tools_subset.is_empty() && !tools_subset.contains(&tc.function.name) {
            denied_msgs.push(ChatMessage {
                message_id: Uuid::new_v4().to_string(),
                role: "tool".to_string(),
                tool_call_id: tc.id.clone(),
                tool_failed: Some(true),
                content: ChatContent::SimpleText(format!(
                    "Tool '{}' not allowed in this subchat",
                    tc.function.name
                )),
                ..Default::default()
            });
        } else {
            allowed.push(tc.clone());
        }
    }

    let thread = ThreadParams {
        id: format!("subchat-{}", Uuid::new_v4()),
        model: model_id.to_string(),
        ..Default::default()
    };

    if let (Some(tx_toolid), Some(tx_chatid)) = (&tx_toolid_mb, &tx_chatid_mb) {
        let subchat_tx = ccx.lock().await.subchat_tx.clone();
        for tc in &allowed {
            let paths = extract_paths_from_tool_args(&tc.function.name, &tc.function.arguments);
            let tool_msg = json!({
                "tool_call_id": tx_toolid,
                "subchat_id": format!("{}/tool:{}", tx_chatid, tc.function.name),
                "tool_call": {
                    "name": tc.function.name,
                    "arguments": tc.function.arguments
                },
                "attached_files": paths
            });
            let _ = subchat_tx.lock().await.send(tool_msg);
        }
    }

    let (mut tool_results, _) = execute_tools(
        gcx.clone(),
        &allowed,
        &messages,
        &thread,
        ChatMode::AGENT,
        ExecuteToolsOptions::default(),
    )
    .await;

    for tc in &tool_calls {
        let answered = denied_msgs
            .iter()
            .chain(tool_results.iter())
            .any(|m| m.tool_call_id == tc.id);
        if !answered {
            tool_results.push(ChatMessage {
                message_id: Uuid::new_v4().to_string(),
                role: "tool".to_string(),
                tool_call_id: tc.id.clone(),
                tool_failed: Some(false),
                content: ChatContent::SimpleText("Tool executed with no output.".to_string()),
                ..Default::default()
            });
        }
    }

    messages.extend(denied_msgs);
    messages.extend(tool_results);
    Ok(messages)
}

async fn subchat_stream(
    ccx: Arc<AMutex<AtCommandsContext>>,
    model_id: &str,
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDesc>,
    prepend_system_prompt: bool,
    temperature: Option<f32>,
    max_new_tokens: usize,
    n: usize,
    reasoning_effort: Option<ReasoningEffort>,
    only_deterministic_messages: bool,
) -> Result<Vec<Vec<ChatMessage>>, String> {
    let (gcx, effective_n_ctx) = {
        let ccx_locked = ccx.lock().await;
        (ccx_locked.global_context.clone(), ccx_locked.n_ctx)
    };

    let caps = try_load_caps_quickly_if_not_present(gcx.clone(), 0)
        .await
        .map_err(|e| format!("no caps: {:?}", e))?;
    let model_rec = resolve_chat_model(caps, model_id)?;

    let tokenizer_arc = crate::tokens::cached_tokenizer(gcx.clone(), &model_rec.base).await?;
    let t = HasTokenizerAndEot::new(tokenizer_arc);

    let capped_n_ctx = effective_n_ctx.min(model_rec.base.n_ctx);

    let meta = ChatMeta {
        chat_id: Uuid::new_v4().to_string(),
        chat_mode: ChatMode::AGENT,
        chat_remote: false,
        current_config_file: String::new(),
        context_tokens_cap: Some(capped_n_ctx),
        include_project_info: true,
        request_attempt_id: Uuid::new_v4().to_string(),
    };

    let mut parameters = SamplingParameters {
        max_new_tokens,
        temperature,
        n: Some(n),
        reasoning_effort,
        ..Default::default()
    };

    let options = ChatPrepareOptions {
        prepend_system_prompt,
        allow_at_commands: false,
        allow_tool_prerun: false,
        supports_tools: model_rec.supports_tools,
    };

    if only_deterministic_messages {
        return Ok(vec![messages]);
    }

    let prepared = prepare_chat_passthrough(
        gcx.clone(),
        ccx.clone(),
        &t,
        messages.clone(),
        model_id,
        tools,
        &meta,
        &mut parameters,
        &options,
        &None,
    )
    .await?;

    let t1 = std::time::Instant::now();

    let params = StreamRunParams {
        prompt: prepared.prompt,
        model_rec: model_rec.base.clone(),
        sampling: parameters,
        meta: if model_rec.base.support_metadata {
            Some(meta)
        } else {
            None
        },
        abort_flag: None,
    };

    let mut collector = NoopCollector;
    let results = run_llm_stream(gcx.clone(), params, n, &mut collector).await?;

    info!(
        "stream generation took {:?}ms",
        t1.elapsed().as_millis() as i32
    );

    convert_results_to_messages(results, messages)
}

fn convert_results_to_messages(
    results: Vec<ChoiceFinal>,
    original_messages: Vec<ChatMessage>,
) -> Result<Vec<Vec<ChatMessage>>, String> {
    if results.is_empty() {
        return Ok(vec![original_messages]);
    }

    let mut all_choices = vec![];
    for result in results {
        let tool_calls: Option<Vec<_>> = if result.tool_calls_raw.is_empty() {
            None
        } else {
            let parsed: Vec<_> = result
                .tool_calls_raw
                .iter()
                .filter_map(|tc| normalize_tool_call(tc))
                .collect();
            if parsed.is_empty() {
                None
            } else {
                Some(parsed)
            }
        };

        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::SimpleText(result.content),
            tool_calls,
            reasoning_content: if result.reasoning.is_empty() {
                None
            } else {
                Some(result.reasoning)
            },
            thinking_blocks: if result.thinking_blocks.is_empty() {
                None
            } else {
                Some(result.thinking_blocks)
            },
            usage: result.usage,
            ..Default::default()
        };

        let mut extended = original_messages.clone();
        extended.push(msg);
        all_choices.push(extended);
    }

    Ok(all_choices)
}

fn update_usage_from_messages(usage: &mut ChatUsage, messages: &Vec<Vec<ChatMessage>>) {
    // even if n_choices > 1, usage is identical in each Vec<ChatMessage>, so we could take the first one
    if let Some(message_0) = messages.get(0) {
        if let Some(last_message) = message_0.last() {
            if let Some(u) = last_message.usage.as_ref() {
                usage.total_tokens += u.total_tokens;
                usage.completion_tokens += u.completion_tokens;
                usage.prompt_tokens += u.prompt_tokens;
            }
        }
    }
}

pub async fn subchat_single(
    ccx: Arc<AMutex<AtCommandsContext>>,
    model_id: &str,
    messages: Vec<ChatMessage>,
    tools_subset: Option<Vec<String>>,
    _tool_choice: Option<String>,
    only_deterministic_messages: bool,
    temperature: Option<f32>,
    max_new_tokens: Option<usize>,
    n: usize,
    reasoning_effort: Option<ReasoningEffort>,
    prepend_system_prompt: bool,
    usage_collector_mb: Option<&mut ChatUsage>,
    tx_toolid_mb: Option<String>,
    tx_chatid_mb: Option<String>,
) -> Result<Vec<Vec<ChatMessage>>, String> {
    let gcx = {
        let ccx_locked = ccx.lock().await;
        ccx_locked.global_context.clone()
    };

    info!("tools_subset {:?}", tools_subset);

    let tools_desclist: Vec<ToolDesc> = {
        let tools_turned_on_by_cmdline = get_available_tools(gcx.clone())
            .await
            .iter()
            .map(|tool| tool.tool_description())
            .collect::<Vec<_>>();

        info!(
            "tools_turned_on_by_cmdline {:?}",
            tools_turned_on_by_cmdline
                .iter()
                .map(|tool| { &tool.name })
                .collect::<Vec<_>>()
        );

        match tools_subset {
            Some(ref tools_subset) => tools_turned_on_by_cmdline
                .into_iter()
                .filter(|tool| tools_subset.contains(&tool.name))
                .collect(),
            None => tools_turned_on_by_cmdline,
        }
    };

    info!(
        "tools_on_intersection {:?}",
        tools_desclist
            .iter()
            .map(|tool| { &tool.name })
            .collect::<Vec<_>>()
    );

    let tools = tools_desclist
        .into_iter()
        .filter(|x| x.is_supported_by(model_id))
        .collect::<Vec<_>>();

    let max_new_tokens = max_new_tokens.unwrap_or(MAX_NEW_TOKENS);

    let results = subchat_stream(
        ccx.clone(),
        model_id,
        messages.clone(),
        tools,
        prepend_system_prompt,
        temperature,
        max_new_tokens,
        n,
        reasoning_effort,
        only_deterministic_messages,
    )
    .await?;

    if let Some(usage_collector) = usage_collector_mb {
        update_usage_from_messages(usage_collector, &results);
    }

    if let Some(tx_chatid) = tx_chatid_mb {
        if let Some(tx_toolid) = tx_toolid_mb {
            let subchat_tx = ccx.lock().await.subchat_tx.clone();
            for (i, choice) in results.iter().enumerate() {
                let cid = if results.len() > 1 {
                    format!("{}-choice{}", tx_chatid, i)
                } else {
                    tx_chatid.clone()
                };
                if let Some(last_msg) = choice.last() {
                    let message = json!({"tool_call_id": tx_toolid, "subchat_id": cid, "add_message": last_msg});
                    let _ = subchat_tx.lock().await.send(message);
                }
            }
        }
    }

    Ok(results)
}

pub async fn subchat(
    ccx: Arc<AMutex<AtCommandsContext>>,
    model_id: &str,
    messages: Vec<ChatMessage>,
    tools_subset: Vec<String>,
    wrap_up_depth: usize,
    wrap_up_tokens_cnt: usize,
    wrap_up_prompt: &str,
    wrap_up_n: usize,
    temperature: Option<f32>,
    reasoning_effort: Option<ReasoningEffort>,
    tx_toolid_mb: Option<String>,
    tx_chatid_mb: Option<String>,
    prepend_system_prompt: Option<bool>,
) -> Result<Vec<Vec<ChatMessage>>, String> {
    let mut messages = messages.clone();
    let mut usage_collector = ChatUsage {
        ..Default::default()
    };
    let mut tx_chatid_mb = tx_chatid_mb.clone();
    // for attempt in attempt_n
    {
        // keep session
        let mut step_n = 0;
        loop {
            if has_final_answer(&messages) {
                break;
            }
            let last_message = messages.last().unwrap();
            if last_message.role == "assistant" && last_message.tool_calls.as_ref().map_or(false, |tc| !tc.is_empty()) {
                // have tool calls, let's see if we need to wrap up or not
                if step_n >= wrap_up_depth {
                    break;
                }
                if let Some(usage) = &last_message.usage {
                    if usage.prompt_tokens + usage.completion_tokens > wrap_up_tokens_cnt {
                        break;
                    }
                }
            }
            messages = subchat_single(
                ccx.clone(),
                model_id,
                messages.clone(),
                Some(tools_subset.clone()),
                Some("auto".to_string()),
                false,
                temperature,
                None,
                1,
                reasoning_effort.clone(),
                prepend_system_prompt.unwrap_or(false),
                Some(&mut usage_collector),
                tx_toolid_mb.clone(),
                tx_chatid_mb.clone(),
            )
            .await?[0]
                .clone();
            let assistant_msg = messages.iter().rev()
                .find(|m| m.role == "assistant")
                .unwrap();
            let content = if let Some(tool_calls) = &assistant_msg.tool_calls {
                let items: Vec<String> = tool_calls.iter()
                    .map(|tc| {
                        let args_short = truncate_text(&tc.function.arguments, 50);
                        format!("{}({})", tc.function.name, args_short)
                    })
                    .collect();
                items.join("\n")
            } else {
                let text = assistant_msg.content.content_text_only();
                format!("🤖 {}", truncate_text(&text, 50))
            };
            let tx_chatid = format!("{}/{}: {}", step_n + 1, wrap_up_depth, content);
            info!("subchat progress: {tx_chatid}");
            tx_chatid_mb = Some(tx_chatid.clone());

            if let Some(tx_toolid) = &tx_toolid_mb {
                let subchat_tx = ccx.lock().await.subchat_tx.clone();
                let _ = subchat_tx.lock().await.send(json!({
                    "tool_call_id": tx_toolid,
                    "subchat_id": tx_chatid,
                }));
            }

            messages = execute_pending_tool_calls(
                ccx.clone(),
                model_id,
                messages,
                &tools_subset,
                tx_toolid_mb.clone(),
                tx_chatid_mb.clone(),
            )
            .await?;
            step_n += 1;
        }
        // result => session
    }
    messages = execute_pending_tool_calls(
        ccx.clone(),
        model_id,
        messages,
        &tools_subset,
        tx_toolid_mb.clone(),
        tx_chatid_mb.clone(),
    )
    .await?;
    messages.push(ChatMessage::new(
        "user".to_string(),
        wrap_up_prompt.to_string(),
    ));
    let choices = subchat_single(
        ccx.clone(),
        model_id,
        messages,
        Some(vec![]),
        Some("none".to_string()),
        false,
        temperature,
        None,
        wrap_up_n,
        reasoning_effort.clone(),
        prepend_system_prompt.unwrap_or(false),
        Some(&mut usage_collector),
        tx_toolid_mb.clone(),
        tx_chatid_mb.clone(),
    )
    .await?;

    if let Some(tx_toolid) = &tx_toolid_mb {
        let subchat_tx = ccx.lock().await.subchat_tx.clone();
        let reset_msg = json!({
            "tool_call_id": tx_toolid,
            "subchat_id": "",
            "finished": true
        });
        let _ = subchat_tx.lock().await.send(reset_msg);
    }

    Ok(choices)
}
