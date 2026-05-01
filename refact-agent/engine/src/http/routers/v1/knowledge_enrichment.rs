use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock as ARwLock;
use regex::Regex;
use axum::extract::{Extension, Path};
use axum::response::IntoResponse;
use axum::Json;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};

use crate::call_validation::{ChatContent, ChatMessage, ContextFile};
use crate::global_context::GlobalContext;
use crate::memories::memories_search;
use crate::subchat::{resolve_subchat_config, run_subchat};
use crate::yaml_configs::customization_registry::get_subagent_config;

static PATH_IN_CARD_RE: OnceLock<Regex> = OnceLock::new();
static TITLE_IN_CARD_RE: OnceLock<Regex> = OnceLock::new();
static CODE_FENCE_RE: OnceLock<Regex> = OnceLock::new();
static TOOL_PATH_LINE_RE: OnceLock<Regex> = OnceLock::new();
static TITLE_ICON_RE: OnceLock<Regex> = OnceLock::new();
static KIND_ICON_RE: OnceLock<Regex> = OnceLock::new();
static RELATED_BULLET_RE: OnceLock<Regex> = OnceLock::new();
static LINE_RANGE_SUFFIX_RE: OnceLock<Regex> = OnceLock::new();

fn path_in_card_re() -> &'static Regex {
    PATH_IN_CARD_RE.get_or_init(|| Regex::new(r"Memory file: (.+)").unwrap())
}

fn title_in_card_re() -> &'static Regex {
    TITLE_IN_CARD_RE.get_or_init(|| Regex::new(r"(?m)^Title: (.+)$").unwrap())
}

fn code_fence_re() -> &'static Regex {
    CODE_FENCE_RE.get_or_init(|| Regex::new(r"```[\s\S]*?```").unwrap())
}

fn tool_path_line_re() -> &'static Regex {
    TOOL_PATH_LINE_RE.get_or_init(|| Regex::new(r"(?m)^📄\s+(.+)$").unwrap())
}

fn title_icon_re() -> &'static Regex {
    TITLE_ICON_RE.get_or_init(|| Regex::new(r"(?m)^📌\s+(.+)$").unwrap())
}

fn kind_icon_re() -> &'static Regex {
    KIND_ICON_RE.get_or_init(|| Regex::new(r"(?m)^📦\s+(.+)$").unwrap())
}

fn related_bullet_re() -> &'static Regex {
    RELATED_BULLET_RE.get_or_init(|| Regex::new(r"(?m)^-\s+(.+?)\s+\(([^()\n]+)\)\s*$").unwrap())
}

fn line_range_suffix_re() -> &'static Regex {
    LINE_RANGE_SUFFIX_RE
        .get_or_init(|| Regex::new(r"^(?P<path>.+):(?P<line1>\d+)-(?P<line2>\d+)$").unwrap())
}

fn format_enrichment_card(m: &crate::memories::MemoRecord) -> String {
    let mut out = String::new();
    out.push_str("# Related memory (short form)\n");
    out.push_str("Note: this is a heuristic match and may be unrelated to the actual problem.\n\n");
    if let Some(title) = &m.title {
        out.push_str(&format!("Title: {}\n", title));
    }
    if let Some(kind) = &m.kind {
        out.push_str(&format!("Kind: {}\n", kind));
    }
    if let Some(score) = m.score {
        out.push_str(&format!("Relevance: {:.0}%\n", score * 100.0));
    }
    if !m.tags.is_empty() {
        out.push_str(&format!("Tags: {}\n", m.tags.join(", ")));
    }
    if let Some(path) = &m.file_path {
        out.push_str(&format!("Memory file: {}\n", path.display()));
        out.push_str(&format!(
            "To load full content: call `cat(paths=\"{}\")`\n\n",
            path.display()
        ));
    }
    let snippet: String = m.content.chars().take(900).collect();
    out.push_str(&snippet);
    if m.content.chars().count() > 900 {
        out.push_str("\n\n[TRUNCATED]\n");
    }
    out
}

const KNOWLEDGE_TOP_N: usize = 3;
const TRAJECTORY_TOP_N: usize = 2;
const KNOWLEDGE_SCORE_THRESHOLD: f32 = 0.75;
const FORCED_KNOWLEDGE_SCORE_THRESHOLD: f32 = 0.50;
const KNOWLEDGE_ENRICHMENT_MARKER: &str = "knowledge_enrichment";
const MAX_QUERY_LENGTH: usize = 2000;

pub async fn enrich_messages_with_knowledge(
    gcx: Arc<ARwLock<GlobalContext>>,
    messages: &mut Vec<ChatMessage>,
    current_chat_id: Option<&str>,
    force_enrichment: bool,
) {
    let last_user_idx = match messages.iter().rposition(|m| m.role == "user") {
        Some(idx) => idx,
        None => return,
    };
    let query_raw = messages[last_user_idx].content.content_text_only();

    if has_knowledge_enrichment_near(messages, last_user_idx) {
        return;
    }

    let query_normalized = normalize_query(&query_raw);

    if !should_enrich(messages, &query_raw, &query_normalized, force_enrichment) {
        return;
    }

    let existing_paths = get_existing_context_file_paths(messages);

    let score_threshold = if force_enrichment {
        FORCED_KNOWLEDGE_SCORE_THRESHOLD
    } else {
        KNOWLEDGE_SCORE_THRESHOLD
    };

    if let Some(knowledge_context) = create_knowledge_context(
        gcx,
        &query_normalized,
        &existing_paths,
        current_chat_id,
        score_threshold,
    )
    .await
    {
        messages.insert(last_user_idx, knowledge_context);
        tracing::info!(
            "Injected knowledge context before user message at position {}",
            last_user_idx
        );
    }
}

fn normalize_query(query: &str) -> String {
    let normalized = code_fence_re().replace_all(query, " [code] ").to_string();
    let normalized = normalized.trim();
    if normalized.len() > MAX_QUERY_LENGTH {
        normalized.chars().take(MAX_QUERY_LENGTH).collect()
    } else {
        normalized.to_string()
    }
}

fn should_enrich(
    messages: &[ChatMessage],
    query_raw: &str,
    query_normalized: &str,
    force_enrichment: bool,
) -> bool {
    let trimmed = query_raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('@') || trimmed.starts_with('/') {
        return false;
    }
    if force_enrichment {
        tracing::info!("Knowledge enrichment: explicitly enabled for later turn");
        return true;
    }
    let user_message_count = messages.iter().filter(|m| m.role == "user").count();
    if user_message_count == 1 {
        tracing::info!("Knowledge enrichment: first user message");
        return true;
    }
    let strong = count_strong_signals(query_raw);
    let weak = count_weak_signals(query_raw, query_normalized);
    if strong >= 1 {
        tracing::info!("Knowledge enrichment: {} strong signal(s)", strong);
        return true;
    }
    if weak >= 2 && query_normalized.len() >= 20 {
        tracing::info!("Knowledge enrichment: {} weak signal(s)", weak);
        return true;
    }
    false
}

fn count_strong_signals(query: &str) -> usize {
    let query_lower = query.to_lowercase();
    let mut count = 0;
    let error_keywords = [
        "error",
        "panic",
        "exception",
        "traceback",
        "stack trace",
        "segfault",
        "failed",
        "unable to",
        "cannot",
        "doesn't work",
        "does not work",
        "broken",
        "bug",
        "crash",
    ];
    if error_keywords.iter().any(|kw| query_lower.contains(kw)) {
        count += 1;
    }
    let file_extensions = [
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".cpp", ".c", ".h",
    ];
    let config_files = [
        "cargo.toml",
        "package.json",
        "tsconfig",
        "pyproject",
        ".yaml",
        ".yml",
        ".toml",
    ];
    if file_extensions.iter().any(|ext| query_lower.contains(ext))
        || config_files.iter().any(|f| query_lower.contains(f))
    {
        count += 1;
    }
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let path_re = PATH_RE.get_or_init(|| Regex::new(r"\b[\w-]+/[\w-]+(?:/[\w.-]+)*\b").unwrap());
    if path_re.is_match(query) {
        count += 1;
    }
    if query.contains("::") || query.contains("->") || query.contains("`") {
        count += 1;
    }
    let retrieval_phrases = [
        "search",
        "find",
        "where is",
        "which file",
        "look up",
        "in this repo",
        "in the codebase",
        "in the project",
    ];
    if retrieval_phrases.iter().any(|p| query_lower.contains(p)) {
        count += 1;
    }
    count
}

fn count_weak_signals(query_raw: &str, query_normalized: &str) -> usize {
    let mut count = 0;
    if query_raw.contains('?') {
        count += 1;
    }
    let query_lower = query_raw.trim().to_lowercase();
    let question_starters = [
        "how",
        "why",
        "what",
        "where",
        "when",
        "can",
        "should",
        "could",
        "would",
        "is there",
        "are there",
    ];
    if question_starters.iter().any(|s| query_lower.starts_with(s)) {
        count += 1;
    }
    if query_normalized.len() >= 80 {
        count += 1;
    }
    count
}

async fn create_knowledge_context(
    gcx: Arc<ARwLock<GlobalContext>>,
    query_text: &str,
    existing_paths: &HashSet<String>,
    current_chat_id: Option<&str>,
    score_threshold: f32,
) -> Option<ChatMessage> {
    let memories = memories_search(
        gcx.clone(),
        query_text,
        KNOWLEDGE_TOP_N,
        TRAJECTORY_TOP_N,
        current_chat_id,
    )
    .await
    .ok()?;

    let high_score_memories: Vec<_> = memories
        .into_iter()
        .filter(|m| m.score.unwrap_or(0.0) >= score_threshold)
        .filter(|m| {
            if let Some(path) = &m.file_path {
                !existing_paths.contains(&path.to_string_lossy().to_string())
            } else {
                true
            }
        })
        .collect();

    if high_score_memories.is_empty() {
        return None;
    }

    tracing::info!(
        "Knowledge enrichment: {} memories passed threshold {}",
        high_score_memories.len(),
        score_threshold
    );

    let context_files: Vec<ContextFile> = high_score_memories
        .iter()
        .filter_map(|memo| {
            let file_path = memo.file_path.as_ref()?;
            let card = format_enrichment_card(memo);
            let line_count = card.lines().count().max(1);
            Some(ContextFile {
                file_name: file_path.to_string_lossy().to_string(),
                file_content: card,
                line1: 1,
                line2: line_count,
                file_rev: None,
                symbols: vec![],
                gradient_type: -1,
                usefulness: 80.0 + (memo.score.unwrap_or(0.75) * 20.0),
                skip_pp: true,
            })
        })
        .collect();

    if context_files.is_empty() {
        return None;
    }

    Some(ChatMessage {
        role: "context_file".to_string(),
        content: ChatContent::ContextFiles(context_files),
        tool_call_id: KNOWLEDGE_ENRICHMENT_MARKER.to_string(),
        ..Default::default()
    })
}

fn has_knowledge_enrichment_near(messages: &[ChatMessage], user_idx: usize) -> bool {
    let search_start = user_idx.saturating_sub(2);
    let search_end = (user_idx + 2).min(messages.len());
    for i in search_start..search_end {
        if messages[i].role == "context_file"
            && messages[i].tool_call_id == KNOWLEDGE_ENRICHMENT_MARKER
        {
            tracing::info!("Skipping enrichment - already enriched at position {}", i);
            return true;
        }
    }
    false
}

fn get_existing_context_file_paths(messages: &[ChatMessage]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for msg in messages {
        if msg.role == "context_file" {
            let files: Vec<ContextFile> = match &msg.content {
                ChatContent::ContextFiles(files) => files.clone(),
                ChatContent::SimpleText(text) => {
                    serde_json::from_str::<Vec<ContextFile>>(text).unwrap_or_default()
                }
                _ => vec![],
            };
            for file in files {
                paths.insert(file.file_name.clone());
            }
        }
    }
    paths
}

/// Returns all directories that memory/trajectory files may legitimately live in.
async fn get_allowed_enrichment_dirs(gcx: Arc<ARwLock<GlobalContext>>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let config_dir = gcx.read().await.config_dir.clone();
    dirs.push(config_dir.clone());

    let traj_dirs = crate::chat::trajectories::get_all_trajectories_dirs(gcx.clone()).await;
    dirs.extend(traj_dirs);

    let project_dirs = crate::files_correction::get_project_dirs(gcx.clone()).await;
    for pd in project_dirs {
        dirs.push(pd.join(".refact"));
    }

    dirs
}

fn is_path_in_allowed_dirs(path: &std::path::Path, allowed: &[PathBuf]) -> bool {
    allowed.iter().any(|root| path.starts_with(root))
}

fn strip_line_range_suffix(path: &str) -> String {
    let trimmed = path.trim();
    line_range_suffix_re()
        .captures(trimmed)
        .and_then(|caps| caps.name("path").map(|m| m.as_str().trim().to_string()))
        .unwrap_or_else(|| trimmed.to_string())
}

fn title_from_section(section: &str) -> Option<String> {
    title_in_card_re()
        .captures(section)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .or_else(|| {
            title_icon_re()
                .captures(section)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
        })
}

fn kind_from_section_or_path(section: &str, path_str: &str) -> String {
    let raw_kind = kind_icon_re()
        .captures(section)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_lowercase())
        .filter(|kind| !kind.is_empty())
        .unwrap_or_else(|| {
            if path_str.contains("trajectories") {
                "trajectory".to_string()
            } else {
                "memory".to_string()
            }
        });

    match raw_kind.as_str() {
        "trajectory" => "trajectory".to_string(),
        "file" => "file".to_string(),
        _ => "memory".to_string(),
    }
}

fn push_enrichment_item(
    items: &mut Vec<EnrichmentItem>,
    seen_paths: &mut HashSet<String>,
    allowed_dirs: &[PathBuf],
    raw_path: &str,
    label: Option<String>,
    kind: String,
    content: String,
) {
    let path_str = strip_line_range_suffix(raw_path);

    if path_str.is_empty() || seen_paths.contains(&path_str) {
        return;
    }

    let path = std::path::Path::new(&path_str);
    if !is_path_in_allowed_dirs(path, allowed_dirs) {
        tracing::warn!(
            "preview: skipping enrichment path outside allowed roots: {}",
            path_str
        );
        return;
    }

    let label = label
        .filter(|s| !s.trim().is_empty())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| path_str.clone());
    let content: String = content.chars().take(900).collect();
    let line_count = content.lines().count().max(1);

    seen_paths.insert(path_str.clone());

    items.push(EnrichmentItem {
        kind,
        label,
        context_file: ContextFile {
            file_name: path_str,
            file_content: content,
            line1: 1,
            line2: line_count,
            file_rev: None,
            symbols: vec![],
            gradient_type: -1,
            usefulness: 85.0,
            skip_pp: true,
        },
    });
}

/// Extract enrichment items from tool result messages produced by the knowledge tool.
/// Content comes directly from tool results (server-generated) — no re-reading from disk.
/// Paths are validated against allowed directories.
fn extract_items_from_tool_results(
    messages: &[ChatMessage],
    allowed_dirs: &[PathBuf],
) -> Vec<EnrichmentItem> {
    let path_re = path_in_card_re();
    let title_re = title_in_card_re();

    let mut items: Vec<EnrichmentItem> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    for msg in messages {
        if msg.role != "tool" {
            continue;
        }
        let text = match &msg.content {
            ChatContent::SimpleText(t) => t.as_str(),
            _ => continue,
        };

        for section in text.split("# Related memory").skip(1) {
            let path_str = match path_re
                .captures(section)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
            {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            let label = title_re
                .captures(section)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .or_else(|| title_from_section(section));

            let card = format!("# Related memory{}", section);
            let kind = kind_from_section_or_path(section, &path_str);
            push_enrichment_item(
                &mut items,
                &mut seen_paths,
                allowed_dirs,
                &path_str,
                label,
                kind,
                card,
            );
        }

        for section in text.split("\n---\n") {
            let path_str = match tool_path_line_re()
                .captures(section)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
            {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            let label = title_from_section(section);
            let kind = kind_from_section_or_path(section, &path_str);
            push_enrichment_item(
                &mut items,
                &mut seen_paths,
                allowed_dirs,
                &path_str,
                label,
                kind,
                section.trim().to_string(),
            );
        }

        for caps in related_bullet_re().captures_iter(text) {
            let label = caps.get(1).map(|m| m.as_str().trim().to_string());
            let path_str = match caps.get(2).map(|m| m.as_str().trim().to_string()) {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };
            let content = label
                .as_ref()
                .map(|l| {
                    format!(
                        "# Related memory (short form)\nTitle: {}\nMemory file: {}",
                        l, path_str
                    )
                })
                .unwrap_or_else(|| {
                    format!("# Related memory (short form)\nMemory file: {}", path_str)
                });
            let kind = kind_from_section_or_path(text, &path_str);
            push_enrichment_item(
                &mut items,
                &mut seen_paths,
                allowed_dirs,
                &path_str,
                label,
                kind,
                content,
            );
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: "tool".to_string(),
            content: ChatContent::SimpleText(content.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn extract_items_from_tool_results_parses_knowledge_tool_blocks() {
        let messages = vec![tool_message(
            "📄 /tmp/project/.refact/knowledge/memory.md:1-3\n📌 Memory Title\n📦 decision\nbody\n\n---\n",
        )];
        let items =
            extract_items_from_tool_results(&messages, &[PathBuf::from("/tmp/project/.refact")]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "Memory Title");
        assert_eq!(items[0].kind, "memory");
        assert_eq!(
            items[0].context_file.file_name,
            "/tmp/project/.refact/knowledge/memory.md"
        );
    }

    #[test]
    fn extract_items_from_tool_results_parses_related_memory_bullets() {
        let messages = vec![tool_message(
            "## Related memories (short form)\n\n- Related Title (/tmp/project/.refact/knowledge/related.md)\n  short desc\n",
        )];
        let items =
            extract_items_from_tool_results(&messages, &[PathBuf::from("/tmp/project/.refact")]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "Related Title");
        assert_eq!(
            items[0].context_file.file_name,
            "/tmp/project/.refact/knowledge/related.md"
        );
    }
}

/// Request body for the manual memory enrichment preview endpoint.
#[derive(Deserialize)]
pub struct MemoryEnrichmentPreviewRequest {
    pub text: String,
}

/// A single enrichment item returned to the frontend for wand-preview chip rendering.
#[derive(Serialize)]
pub struct EnrichmentItem {
    pub kind: String,
    pub label: String,
    pub context_file: ContextFile,
}

/// Response shape for the wand-preview endpoint.
#[derive(Serialize)]
pub struct MemoryEnrichmentPreviewResponse {
    pub query_used: String,
    pub rewritten_text: String,
    pub items: Vec<EnrichmentItem>,
}

/// POST /v1/chats/:chat_id/memory-enrichment/preview
pub async fn handle_v1_memory_enrichment_preview(
    Path(_chat_id): Path<String>,
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    Json(payload): Json<MemoryEnrichmentPreviewRequest>,
) -> impl IntoResponse {
    let text = payload.text.trim().to_string();
    if text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "text must not be empty"})),
        )
            .into_response();
    }

    let query = if text.len() > MAX_QUERY_LENGTH {
        text.chars().take(MAX_QUERY_LENGTH).collect::<String>()
    } else {
        text.clone()
    };

    match model_gather_and_rewrite(gcx.clone(), &query).await {
        Ok((rewritten_text, items)) => {
            let resp = MemoryEnrichmentPreviewResponse {
                query_used: query,
                rewritten_text,
                items,
            };
            (
                StatusCode::OK,
                Json(serde_json::to_value(resp).unwrap_or_default()),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!("memory enrichment preview failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e})),
            )
                .into_response()
        }
    }
}

const ENRICHMENT_SUBAGENT_ID: &str = "memory_enrichment_rewrite";

async fn model_gather_and_rewrite(
    gcx: Arc<ARwLock<GlobalContext>>,
    query: &str,
) -> Result<(String, Vec<EnrichmentItem>), String> {
    let system_prompt = get_subagent_config(gcx.clone(), ENRICHMENT_SUBAGENT_ID, None)
        .await
        .and_then(|c| c.messages.system_prompt)
        .unwrap_or_else(|| {
            "Search for relevant memories using the knowledge tool, then output JSON: \
            {\"rewritten_text\": \"...\"}"
                .to_string()
        });

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: ChatContent::SimpleText(system_prompt),
            ..Default::default()
        },
        ChatMessage {
            role: "user".to_string(),
            content: ChatContent::SimpleText(query.to_string()),
            ..Default::default()
        },
    ];

    let config = resolve_subchat_config(
        gcx.clone(),
        ENRICHMENT_SUBAGENT_ID,
        false,
        None,
        None,
        None,
        None,
        None,
        Some(vec!["knowledge".to_string()]),
        4,
        false,
        None,
        "agent".to_string(),
    )
    .await
    .map_err(|e| format!("config: {}", e))?;

    let result = run_subchat(gcx.clone(), messages, config)
        .await
        .map_err(|e| format!("subchat: {}", e))?;

    let last_text = result
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(|m| match &m.content {
            ChatContent::SimpleText(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let rewritten_text = parse_rewritten_text(&last_text);

    let allowed_dirs = get_allowed_enrichment_dirs(gcx.clone()).await;
    let mut items = extract_items_from_tool_results(&result.messages, &allowed_dirs);

    items.truncate(5);

    Ok((rewritten_text, items))
}

fn parse_rewritten_text(text: &str) -> String {
    let stripped = {
        let t = text.trim();
        if t.starts_with("```") {
            let inner: Vec<&str> = t.lines().skip(1).collect();
            let last = inner
                .iter()
                .rposition(|l| l.trim() == "```")
                .unwrap_or(inner.len());
            inner[..last].join("\n")
        } else {
            t.to_string()
        }
    };

    let val = serde_json::from_str::<serde_json::Value>(stripped.trim())
        .or_else(|_| crate::json_utils::extract_json_object(text));

    match val {
        Ok(v) => v
            .get("rewritten_text")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}
