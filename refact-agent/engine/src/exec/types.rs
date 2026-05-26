use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SHORT_DESC_MAX_LEN: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecProcessId(pub String);

impl ExecProcessId {
    pub fn new() -> Self {
        Self(format!("exec_{}", Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ExecProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecMode {
    Foreground,
    Background,
    Service,
    Interactive,
}

impl std::fmt::Display for ExecMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecMode::Foreground => write!(f, "foreground"),
            ExecMode::Background => write!(f, "background"),
            ExecMode::Service => write!(f, "service"),
            ExecMode::Interactive => write!(f, "interactive"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecStatus {
    Starting,
    Running,
    Exited { exit_code: Option<i32> },
    Failed { message: String },
    Killed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecOutputStream {
    Stdout,
    Stderr,
    Combined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecOutputChunk {
    pub process_id: ExecProcessId,
    pub seq: u64,
    pub stream: ExecOutputStream,
    pub text: String,
    pub timestamp_ms: u64,
}

pub fn sanitize_short_description(s: &str) -> String {
    let first_line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let cleaned: String = first_line
        .chars()
        .filter_map(|c| {
            if c == '\t' {
                Some(' ')
            } else if c.is_control() || c == '\x7f' {
                None
            } else {
                Some(c)
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= SHORT_DESC_MAX_LEN {
        chars.into_iter().collect()
    } else {
        let mut result: String = chars[..SHORT_DESC_MAX_LEN - 1].iter().collect();
        result.push('\u{2026}');
        result
    }
}

pub fn generate_short_description(command: &str, mode: &ExecMode) -> String {
    if command.trim().is_empty() {
        sanitize_short_description(&mode.to_string())
    } else {
        sanitize_short_description(command)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecProcessMeta {
    pub process_id: ExecProcessId,
    pub mode: ExecMode,
    pub command: String,
    pub short_description: String,
    pub created_at_ms: u64,
}

impl ExecProcessMeta {
    pub fn new(mode: ExecMode, command: String) -> Self {
        let short_description = generate_short_description(&command, &mode);
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            process_id: ExecProcessId::new(),
            mode,
            command,
            short_description,
            created_at_ms,
        }
    }

    pub fn with_short_description(mut self, desc: String) -> Self {
        self.short_description = sanitize_short_description(&desc);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecProcessSnapshot {
    pub meta: ExecProcessMeta,
    pub status: ExecStatus,
}

impl ExecProcessSnapshot {
    pub fn new(meta: ExecProcessMeta) -> Self {
        Self {
            status: ExecStatus::Starting,
            meta,
        }
    }

    pub fn with_status(mut self, status: ExecStatus) -> Self {
        self.status = status;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_id_prefix() {
        let id = ExecProcessId::new();
        assert!(id.as_str().starts_with("exec_"), "ID should start with 'exec_': {}", id);
    }

    #[test]
    fn test_process_id_display() {
        let id = ExecProcessId("exec_abc123".to_string());
        assert_eq!(id.to_string(), "exec_abc123");
    }

    #[test]
    fn test_sanitize_short_description_basic() {
        assert_eq!(sanitize_short_description("hello world"), "hello world");
    }

    #[test]
    fn test_sanitize_short_description_control_chars() {
        let input = "hello\x01\x02world\x7f";
        assert_eq!(sanitize_short_description(input), "helloworld");
    }

    #[test]
    fn test_sanitize_short_description_tabs_become_spaces() {
        let input = "hello\tworld";
        assert_eq!(sanitize_short_description(input), "hello world");
    }

    #[test]
    fn test_sanitize_short_description_capping() {
        let long = "a".repeat(100);
        let result = sanitize_short_description(&long);
        let chars: Vec<char> = result.chars().collect();
        assert_eq!(chars.len(), SHORT_DESC_MAX_LEN);
        assert_eq!(chars[SHORT_DESC_MAX_LEN - 1], '\u{2026}');
    }

    #[test]
    fn test_sanitize_short_description_exactly_at_limit() {
        let s = "a".repeat(SHORT_DESC_MAX_LEN);
        let result = sanitize_short_description(&s);
        assert_eq!(result, s);
    }

    #[test]
    fn test_sanitize_short_description_multiline_takes_first_nonempty() {
        let input = "\n  \nfirst line\nsecond line";
        assert_eq!(sanitize_short_description(input), "first line");
    }

    #[test]
    fn test_sanitize_short_description_trims_whitespace() {
        assert_eq!(sanitize_short_description("  hello  "), "hello");
    }

    #[test]
    fn test_sanitize_short_description_empty_string() {
        assert_eq!(sanitize_short_description(""), "");
    }

    #[test]
    fn test_generate_short_description_from_command() {
        let result = generate_short_description("cargo build --release", &ExecMode::Foreground);
        assert_eq!(result, "cargo build --release");
    }

    #[test]
    fn test_generate_short_description_fallback_to_mode_when_empty() {
        let result = generate_short_description("", &ExecMode::Background);
        assert_eq!(result, "background");
    }

    #[test]
    fn test_generate_short_description_fallback_whitespace_only() {
        let result = generate_short_description("   ", &ExecMode::Service);
        assert_eq!(result, "service");
    }

    #[test]
    fn test_process_meta_uses_command_for_short_desc() {
        let meta = ExecProcessMeta::new(ExecMode::Foreground, "echo hello".to_string());
        assert_eq!(meta.short_description, "echo hello");
        assert_eq!(meta.command, "echo hello");
    }

    #[test]
    fn test_process_meta_custom_short_description() {
        let meta = ExecProcessMeta::new(ExecMode::Service, "nginx".to_string())
            .with_short_description("Web server (nginx)".to_string());
        assert_eq!(meta.short_description, "Web server (nginx)");
        assert_eq!(meta.command, "nginx");
    }

    #[test]
    fn test_snapshot_starts_in_starting_status() {
        let meta = ExecProcessMeta::new(ExecMode::Foreground, "ls".to_string());
        let snap = ExecProcessSnapshot::new(meta);
        assert_eq!(snap.status, ExecStatus::Starting);
    }

    #[test]
    fn test_snapshot_with_status() {
        let meta = ExecProcessMeta::new(ExecMode::Foreground, "ls".to_string());
        let snap = ExecProcessSnapshot::new(meta).with_status(ExecStatus::Exited { exit_code: Some(0) });
        assert_eq!(snap.status, ExecStatus::Exited { exit_code: Some(0) });
    }

    #[test]
    fn test_snapshot_serialization_round_trip() {
        let meta = ExecProcessMeta {
            process_id: ExecProcessId("exec_test123".to_string()),
            mode: ExecMode::Background,
            command: "sleep 10".to_string(),
            short_description: "sleep 10".to_string(),
            created_at_ms: 1_000_000,
        };
        let snap = ExecProcessSnapshot::new(meta)
            .with_status(ExecStatus::Running);

        let json = serde_json::to_string(&snap).expect("serialization failed");
        let deserialized: ExecProcessSnapshot =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(snap, deserialized);
    }

    #[test]
    fn test_exec_status_failed_serialization() {
        let status = ExecStatus::Failed { message: "oom killed".to_string() };
        let json = serde_json::to_string(&status).unwrap();
        let back: ExecStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
    }
}
