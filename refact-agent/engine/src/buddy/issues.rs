use std::path::PathBuf;
use std::sync::Arc;
use regex::Regex;
use tokio::process::Command;
use tokio::sync::RwLock as ARwLock;
use tracing::info;

use crate::global_context::GlobalContext;
use super::diagnostics::DiagnosticContext;
use super::types::BuddyActivity;

const RATE_LIMIT_SECS: u64 = 3600;
const DEDUP_SECS: i64 = 86400;
const ALLOWED_GH_BINARIES: &[&str] = &["gh"];
const ALLOWED_GLAB_BINARIES: &[&str] = &["glab"];

#[derive(Debug)]
pub struct IssueGate {
    pub has_diagnostics: bool,
    pub has_repro_context: bool,
    pub integration_configured: bool,
    pub auto_creation_enabled: bool,
    pub within_rate_limit: bool,
}

pub fn check_issue_gate(gate: &IssueGate) -> bool {
    gate.has_diagnostics
        && gate.has_repro_context
        && gate.integration_configured
        && gate.auto_creation_enabled
        && gate.within_rate_limit
}

pub fn check_manual_issue_gate(gate: &IssueGate) -> bool {
    gate.has_diagnostics && gate.integration_configured
}

fn gate_error(gate: &IssueGate, manual: bool) -> String {
    if !gate.has_diagnostics {
        return "gate blocked: no diagnostic information (need non-empty error with source file or tool name)".to_string();
    }
    if !manual && !gate.has_repro_context {
        return "gate blocked: no reproduction context (source file or tool name required)"
            .to_string();
    }
    if !gate.integration_configured {
        return "gate blocked: no issue tracker integration configured".to_string();
    }
    if !manual && !gate.auto_creation_enabled {
        return "gate blocked: automatic issue creation is disabled in settings".to_string();
    }
    if !manual && !gate.within_rate_limit {
        return "gate blocked: rate limit active (one issue per hour)".to_string();
    }
    "gate blocked: unknown condition".to_string()
}

#[derive(Debug, Clone)]
enum IssueProvider {
    GitHub { binary: String, token: String },
    GitLab { binary: String, token: String },
}

fn validate_binary_name(binary: &str, allowed: &[&str]) -> Result<(), String> {
    let name = std::path::Path::new(binary)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(binary);
    if allowed.contains(&name) {
        Ok(())
    } else {
        Err(format!(
            "binary '{}' is not in the allowed list {:?}",
            binary, allowed
        ))
    }
}

async fn try_read_github(config_dir: &PathBuf) -> Result<Option<IssueProvider>, String> {
    let path = config_dir.join("integrations.d").join("github.yaml");
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let val: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let token = match val.get("gh_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Ok(None),
    };
    let binary = val
        .get("gh_binary_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gh")
        .to_string();
    validate_binary_name(&binary, ALLOWED_GH_BINARIES)?;
    Ok(Some(IssueProvider::GitHub { binary, token }))
}

async fn try_read_gitlab(config_dir: &PathBuf) -> Result<Option<IssueProvider>, String> {
    let path = config_dir.join("integrations.d").join("gitlab.yaml");
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let val: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let token = match val.get("glab_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Ok(None),
    };
    let binary = val
        .get("glab_binary_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("glab")
        .to_string();
    validate_binary_name(&binary, ALLOWED_GLAB_BINARIES)?;
    Ok(Some(IssueProvider::GitLab { binary, token }))
}

async fn detect_remote_host(project_root: &std::path::Path) -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(project_root)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.contains("github.com") {
        Some("github.com".to_string())
    } else if url.contains("gitlab") {
        Some("gitlab.com".to_string())
    } else {
        None
    }
}

async fn detect_provider(
    gcx: Arc<ARwLock<GlobalContext>>,
    project_root: &std::path::Path,
) -> Result<Option<IssueProvider>, String> {
    let remote_host = detect_remote_host(project_root).await;

    let active = crate::files_correction::get_active_project_path(gcx.clone()).await;
    let (config_dirs, global_config_dir) =
        crate::integrations::setting_up_integrations::get_config_dirs(gcx.clone(), &active).await;
    let mut search_dirs: Vec<PathBuf> = config_dirs;
    search_dirs.push(global_config_dir);

    let mut gh_provider: Option<IssueProvider> = None;
    let mut gl_provider: Option<IssueProvider> = None;

    for dir in &search_dirs {
        if gh_provider.is_none() {
            gh_provider = try_read_github(dir).await?;
        }
        if gl_provider.is_none() {
            gl_provider = try_read_gitlab(dir).await?;
        }
    }

    Ok(match remote_host.as_deref() {
        Some("github.com") => gh_provider.or(gl_provider),
        Some("gitlab.com") => gl_provider.or(gh_provider),
        _ => gh_provider.or(gl_provider),
    })
}

pub(crate) fn redact_diagnostic_text(text: &str) -> String {
    let patterns: &[(&str, &str)] = &[
        (r"Bearer [A-Za-z0-9._\-]{8,}", "Bearer [REDACTED]"),
        (r"ghp_[A-Za-z0-9]{10,}", "[REDACTED_GH_TOKEN]"),
        (r"glpat-[A-Za-z0-9_\-]{10,}", "[REDACTED_GL_TOKEN]"),
        (r"sk-[A-Za-z0-9]{20,}", "[REDACTED_SK_TOKEN]"),
    ];
    let mut result = text.to_string();
    for (pattern, replacement) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            result = result.replace(&home, "~");
        }
    }
    if result.chars().count() > 2000 {
        result = result.chars().take(2000).collect();
    }
    result
}

pub(crate) fn sanitize_title(raw: &str) -> String {
    let single: String = raw.chars().filter(|&c| c != '\n' && c != '\r').collect();
    if single.chars().count() > 120 {
        single.chars().take(120).collect()
    } else {
        single
    }
}

pub(crate) fn sanitize_body(raw: &str) -> String {
    let escaped = raw.replace("```", "'''");
    if escaped.chars().count() > 4000 {
        escaped.chars().take(4000).collect()
    } else {
        escaped
    }
}

fn format_issue_body(ctx: &DiagnosticContext) -> String {
    let mut body = format!(
        "**Error type**: {}\n**Severity**: {:?}\n**Collected at**: {}\n\n**Message**:\n```\n{}\n```",
        ctx.error_type, ctx.severity, ctx.collected_at, ctx.error_message
    );
    if let Some(ref file) = ctx.source_file {
        body.push_str(&format!("\n\n**Source file**: `{}`", file));
    }
    if let Some(ref tool) = ctx.tool_name {
        body.push_str(&format!("\n**Tool**: `{}`", tool));
    }
    if let Some(ref chat) = ctx.chat_id {
        body.push_str(&format!("\n**Chat ID**: `{}`", chat));
    }
    body.push_str("\n\n_Auto-created by Buddy diagnostics pipeline._");
    body
}

pub async fn create_issue(
    gcx: Arc<ARwLock<GlobalContext>>,
    context: &DiagnosticContext,
    auto_creation_enabled: bool,
    manual: bool,
    last_issue_at: Option<std::time::Instant>,
    recent_errors: &[(String, chrono::DateTime<chrono::Utc>)],
) -> Result<(String, BuddyActivity), String> {
    let project_root = crate::files_correction::get_project_dirs(gcx.clone())
        .await
        .into_iter()
        .next()
        .ok_or_else(|| "no project root".to_string())?;

    let provider = detect_provider(gcx.clone(), &project_root).await?;

    let gate = IssueGate {
        has_diagnostics: !context.error_message.is_empty()
            && (context.source_file.is_some() || context.tool_name.is_some()),
        has_repro_context: context.source_file.is_some() || context.tool_name.is_some(),
        integration_configured: provider.is_some(),
        auto_creation_enabled,
        within_rate_limit: last_issue_at
            .map(|t| t.elapsed().as_secs() >= RATE_LIMIT_SECS)
            .unwrap_or(true),
    };

    let passed = if manual {
        check_manual_issue_gate(&gate)
    } else {
        check_issue_gate(&gate)
    };

    if !passed {
        return Err(gate_error(&gate, manual));
    }

    let now = chrono::Utc::now();
    for (msg, ts) in recent_errors {
        let age = now.signed_duration_since(*ts).num_seconds();
        if age < DEDUP_SECS && msg == &context.error_message {
            return Err("Duplicate issue suppressed (same error within 24h)".to_string());
        }
    }

    let mut redacted = context.clone();
    redacted.error_message = redact_diagnostic_text(&context.error_message);

    let raw_title = format!(
        "[Buddy] {}: {}",
        context.error_type,
        &context.error_message.chars().take(80).collect::<String>()
    );
    let title = sanitize_title(&raw_title);
    let raw_body = format_issue_body(&redacted);
    let body = sanitize_body(&raw_body);

    let url = run_issue_create(provider.unwrap(), &project_root, &title, &body).await?;

    info!("buddy: created issue {}", url);

    let activity = BuddyActivity {
        icon: "🐛".to_string(),
        title: "Issue created".to_string(),
        description: format!("Auto-created issue: {}", url),
        timestamp: chrono::Utc::now().to_rfc3339(),
        activity_type: "issue_created".to_string(),
    };
    Ok((url, activity))
}

async fn run_issue_create(
    provider: IssueProvider,
    project_root: &std::path::Path,
    title: &str,
    body: &str,
) -> Result<String, String> {
    match provider {
        IssueProvider::GitHub { binary, token } => {
            let out = Command::new(&binary)
                .args(["issue", "create", "--title", title, "--body", body])
                .current_dir(project_root)
                .env("GH_TOKEN", &token)
                .env("GITHUB_TOKEN", &token)
                .stdin(std::process::Stdio::null())
                .output()
                .await
                .map_err(|e| format!("gh failed: {}", e))?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        }
        IssueProvider::GitLab { binary, token } => {
            let out = Command::new(&binary)
                .args(["issue", "create", "--title", title, "--description", body])
                .current_dir(project_root)
                .env("GITLAB_TOKEN", &token)
                .stdin(std::process::Stdio::null())
                .output()
                .await
                .map_err(|e| format!("glab failed: {}", e))?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        }
    }
}
