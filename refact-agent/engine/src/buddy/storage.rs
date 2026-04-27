use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use serde::Serialize;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use super::diagnostics::DiagnosticContext;
use super::runtime_queue::RuntimeQueue;
use super::state::default_buddy_state;
use super::types::BuddyRuntimeEvent;

/// Maximum number of items kept after replay+cap on load. Mirrors the in-memory cap.
const RUNTIME_QUEUE_MAX_ITEMS: usize = 100;

const DEFAULT_MAIN_PROMPT: &str = "You are Buddy, a persistent project companion inside Refact.\nYou help with code tasks, project setup, diagnostics, and keeping things running smoothly.\nYou are friendly, concise, and focused on being genuinely useful.\n";

fn diagnostics_history_path(project_root: &Path) -> PathBuf {
    project_root.join(".refact/buddy/diagnostics.jsonl")
}

fn runtime_queue_path(project_root: &Path) -> PathBuf {
    project_root.join(".refact/buddy/runtime_queue.jsonl")
}

/// Append-only persistence: every mutation writes one JSON line carrying the
/// post-mutation event. On load, latest entry per `id` wins.
pub async fn append_runtime_event(
    project_root: &Path,
    event: &BuddyRuntimeEvent,
) -> Result<(), String> {
    let path = runtime_queue_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
    }
    let line = format!(
        "{}\n",
        serde_json::to_string(event)
            .map_err(|e| format!("Failed to serialize runtime event: {}", e))?
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| format!("Failed to open runtime queue {:?}: {}", path, e))?;
    file.write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Failed to append runtime queue {:?}: {}", path, e))?;
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush runtime queue {:?}: {}", path, e))
}

/// Replay the JSONL file: keep only the most recent entry per id, preserve
/// order of first occurrence, and cap to `RUNTIME_QUEUE_MAX_ITEMS`.
pub async fn load_runtime_queue(project_root: &Path) -> RuntimeQueue {
    let path = runtime_queue_path(project_root);
    let content = match fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(err) if err.kind() == ErrorKind::NotFound => return RuntimeQueue::new(),
        Err(err) => {
            warn!(
                "buddy: failed to read runtime queue at {:?}: {}, starting empty",
                path, err
            );
            return RuntimeQueue::new();
        }
    };

    let mut order: Vec<String> = Vec::new();
    let mut latest: HashMap<String, BuddyRuntimeEvent> = HashMap::new();
    for (idx, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<BuddyRuntimeEvent>(line) {
            Ok(ev) => {
                if !latest.contains_key(&ev.id) {
                    order.push(ev.id.clone());
                }
                latest.insert(ev.id.clone(), ev);
            }
            Err(err) => {
                warn!(
                    "buddy: failed to parse runtime queue line {} in {:?}: {}",
                    idx + 1,
                    path,
                    err
                );
            }
        }
    }

    if order.len() > RUNTIME_QUEUE_MAX_ITEMS {
        let drop_count = order.len() - RUNTIME_QUEUE_MAX_ITEMS;
        order.drain(..drop_count);
    }

    let mut queue = RuntimeQueue::new();
    for id in order {
        if let Some(ev) = latest.remove(&id) {
            queue.items.push_back(ev);
        }
    }
    queue
}

/// Compaction: rewrite the JSONL file from the in-memory queue to drop
/// superseded entries. Called periodically from the background loop.
pub async fn compact_runtime_queue(
    project_root: &Path,
    queue: &RuntimeQueue,
) -> Result<(), String> {
    let path = runtime_queue_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
    }
    let mut buf = String::new();
    for ev in &queue.items {
        let line = serde_json::to_string(ev)
            .map_err(|e| format!("Failed to serialize runtime event: {}", e))?;
        buf.push_str(&line);
        buf.push('\n');
    }
    let tmp = path.with_extension("jsonl.tmp");
    fs::write(&tmp, &buf)
        .await
        .map_err(|e| format!("Failed to write {:?}: {}", tmp, e))?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)
            .await
            .map_err(|e| format!("Failed to remove existing file: {}", e))?;
    }
    fs::rename(&tmp, &path)
        .await
        .map_err(|e| format!("Failed to rename {:?} to {:?}: {}", tmp, path, e))
}

pub async fn atomic_write_json<T: Serialize>(path: &Path, data: &T) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string(data).map_err(|e| e.to_string())?;
    fs::write(&tmp_path, &json)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)
            .await
            .map_err(|e| format!("Failed to remove existing file: {}", e))?;
    }
    fs::rename(&tmp_path, path)
        .await
        .map_err(|e| format!("Failed to rename: {}", e))
}

pub async fn append_diagnostic(project_root: &Path, ctx: &DiagnosticContext) -> Result<(), String> {
    let path = diagnostics_history_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
    }

    let line = format!(
        "{}\n",
        serde_json::to_string(ctx).map_err(|e| format!("Failed to serialize diagnostic: {}", e))?
    );

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| format!("Failed to open diagnostics history {:?}: {}", path, e))?;
    file.write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Failed to append diagnostics history {:?}: {}", path, e))?;
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush diagnostics history {:?}: {}", path, e))?;
    Ok(())
}

pub async fn load_diagnostics(project_root: &Path) -> Result<Vec<DiagnosticContext>, String> {
    Ok(load_diagnostics_inner(project_root, None).await?.into_iter().collect())
}

pub async fn load_recent_diagnostics(
    project_root: &Path,
    limit: usize,
) -> Result<Vec<DiagnosticContext>, String> {
    Ok(load_diagnostics_inner(project_root, Some(limit))
        .await?
        .into_iter()
        .collect())
}

async fn load_diagnostics_inner(
    project_root: &Path,
    limit: Option<usize>,
) -> Result<VecDeque<DiagnosticContext>, String> {
    let path = diagnostics_history_path(project_root);
    let content = match fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(VecDeque::new()),
        Err(err) => {
            return Err(format!(
                "Failed to read diagnostics history {:?}: {}",
                path, err
            ));
        }
    };

    let mut out = VecDeque::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<DiagnosticContext>(line) {
            Ok(ctx) => {
                out.push_back(ctx);
                if let Some(limit) = limit {
                    while out.len() > limit {
                        out.pop_front();
                    }
                }
            }
            Err(err) => {
                warn!(
                    "buddy: failed to parse diagnostic history line {} in {:?}: {}",
                    index + 1,
                    path,
                    err
                );
            }
        }
    }
    Ok(out)
}

pub async fn bootstrap_buddy_storage(project_root: &Path) -> Result<(), String> {
    let buddy_dir = project_root.join(".refact/buddy");
    let dirs = [
        buddy_dir.clone(),
        buddy_dir.join("skills"),
        buddy_dir.join("chats/conversations"),
        buddy_dir.join("chats/workflows"),
    ];
    for dir in &dirs {
        fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("Failed to create dir {:?}: {}", dir, e))?;
    }
    let state_path = buddy_dir.join("state.json");
    if !state_path.exists() {
        let state = default_buddy_state();
        atomic_write_json(&state_path, &state).await?;
    }
    let settings_path = buddy_dir.join("settings.json");
    if !settings_path.exists() {
        let settings = super::settings::BuddySettings::default();
        atomic_write_json(&settings_path, &settings).await?;
    }
    let prompt_path = buddy_dir.join("main_prompt.md");
    if !prompt_path.exists() {
        fs::write(&prompt_path, DEFAULT_MAIN_PROMPT)
            .await
            .map_err(|e| format!("Failed to write main_prompt.md: {}", e))?;
    }
    Ok(())
}
