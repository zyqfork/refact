use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use super::settings::{BuddySettings, HumorLevel};
use super::types::{BuddyBubblePolicy, BuddyRuntimeEvent};

pub const ANALYSIS_TEXT_MIN_CHARS: usize = 20;
pub const ANALYSIS_TEXT_MAX_CHARS: usize = 500;
pub const PER_CHAT_COOLDOWN_SECS: i64 = 300;
pub const GLOBAL_HOURLY_CAP: u32 = 10;

const BUG_KEYWORDS: &[&str] = &[
    "bug", "error", "crash", "panic", "timeout", "exception", "fail", "broken",
];
const INSIGHT_KEYWORDS: &[&str] = &[
    "plan",
    "design",
    "refactor",
    "perf",
    "performance",
    "test",
    "security",
    "architecture",
    "improve",
    "optimize",
    "migrate",
    "rewrite",
    "review",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatReactionKind {
    Humor,
    Insight,
    BugCandidate,
}

#[derive(Debug, Clone)]
pub struct ChatReaction {
    pub kind: ChatReactionKind,
    pub text: String,
}

pub fn settings_allow_chat_reactions(settings: &BuddySettings) -> bool {
    settings.enabled
        && settings.proactive_enabled
        && settings.message_observation_enabled
        && settings.chat_reactions_enabled
        && !settings.quiet_mode
}

pub fn prepare_analysis_text(raw: &str) -> Option<String> {
    let redacted = refact_core::string_utils::redact_sensitive(raw);
    let normalized: String = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() < ANALYSIS_TEXT_MIN_CHARS {
        return None;
    }
    Some(normalized.chars().take(ANALYSIS_TEXT_MAX_CHARS).collect())
}

pub fn classify_chat_reaction(text: &str, settings: &BuddySettings) -> Option<ChatReaction> {
    let lower = text.to_lowercase();
    let kind = if BUG_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        ChatReactionKind::BugCandidate
    } else if INSIGHT_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        ChatReactionKind::Insight
    } else if settings.humor_enabled && settings.humor_level != HumorLevel::Off {
        ChatReactionKind::Humor
    } else {
        return None;
    };
    Some(ChatReaction {
        kind,
        text: text.to_string(),
    })
}

fn message_hash(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub fn build_reaction_event(
    chat_id: &str,
    analysis_text: &str,
    reaction: &ChatReaction,
) -> BuddyRuntimeEvent {
    let (signal_type, ttl_ms, bubble_policy) = match reaction.kind {
        ChatReactionKind::Humor => ("speech_humor", 90_000u64, BuddyBubblePolicy::Ambient),
        ChatReactionKind::Insight => ("speech_insight", 90_000u64, BuddyBubblePolicy::Ambient),
        ChatReactionKind::BugCandidate => {
            ("chat_bug_candidate", 120_000u64, BuddyBubblePolicy::EventOnce)
        }
    };
    let kind_str = match reaction.kind {
        ChatReactionKind::Humor => "humor",
        ChatReactionKind::Insight => "insight",
        ChatReactionKind::BugCandidate => "bug",
    };
    let dedupe_key = format!(
        "chat_reaction:{chat_id}:{kind_str}:{}",
        message_hash(analysis_text)
    );
    BuddyRuntimeEvent {
        id: Uuid::new_v4().to_string(),
        signal_type: signal_type.to_string(),
        title: format!("Chat: {kind_str}"),
        description: None,
        source: "chat_reactions".to_string(),
        status: "info".to_string(),
        progress: None,
        dedupe_key: Some(dedupe_key),
        priority: "normal".to_string(),
        created_at: Utc::now().to_rfc3339(),
        ttl_ms: Some(ttl_ms),
        bubble_policy: Some(bubble_policy),
        speech_text: Some(reaction.text.clone()),
        scene: None,
        duration_hint: None,
        persistent: false,
        controls: vec![],
        chat_id: Some(chat_id.to_string()),
        dismissed: false,
    }
}

pub struct ChatReactionLimiter {
    per_chat_last_at: HashMap<String, DateTime<Utc>>,
    global_hourly_count: u32,
    global_window_start: DateTime<Utc>,
}

impl ChatReactionLimiter {
    pub fn new() -> Self {
        Self {
            per_chat_last_at: HashMap::new(),
            global_hourly_count: 0,
            global_window_start: Utc::now(),
        }
    }

    pub fn allow(&mut self, chat_id: &str, now: DateTime<Utc>) -> bool {
        if (now - self.global_window_start).num_seconds() >= 3600 {
            self.global_hourly_count = 0;
            self.global_window_start = now;
        }
        if self.global_hourly_count >= GLOBAL_HOURLY_CAP {
            return false;
        }
        if let Some(last) = self.per_chat_last_at.get(chat_id) {
            if (now - *last).num_seconds() < PER_CHAT_COOLDOWN_SECS {
                return false;
            }
        }
        self.per_chat_last_at.insert(chat_id.to_string(), now);
        self.global_hourly_count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn settings_allow_all_defaults() {
        assert!(settings_allow_chat_reactions(&BuddySettings::default()));
    }

    #[test]
    fn settings_gate_blocks_each_toggle() {
        let mut s = BuddySettings::default();
        s.enabled = false;
        assert!(!settings_allow_chat_reactions(&s));

        let mut s = BuddySettings::default();
        s.proactive_enabled = false;
        assert!(!settings_allow_chat_reactions(&s));

        let mut s = BuddySettings::default();
        s.message_observation_enabled = false;
        assert!(!settings_allow_chat_reactions(&s));

        let mut s = BuddySettings::default();
        s.chat_reactions_enabled = false;
        assert!(!settings_allow_chat_reactions(&s));
    }

    #[test]
    fn settings_gate_blocks_quiet_mode() {
        let mut s = BuddySettings::default();
        s.quiet_mode = true;
        assert!(!settings_allow_chat_reactions(&s));
    }

    #[test]
    fn prepare_analysis_text_redacts_secret() {
        let raw = "use Bearer sk-MYSECRET123 for auth in the function call";
        let result = prepare_analysis_text(raw).unwrap();
        assert!(!result.contains("sk-MYSECRET123"), "raw secret must not appear");
        assert!(!result.contains("MYSECRET123"), "partial secret must not appear");
    }

    #[test]
    fn prepare_analysis_text_rejects_short() {
        assert!(prepare_analysis_text("too short").is_none());
    }

    #[test]
    fn prepare_analysis_text_truncates_long_input() {
        let raw = "word ".repeat(300);
        let result = prepare_analysis_text(&raw).unwrap();
        assert!(result.chars().count() <= ANALYSIS_TEXT_MAX_CHARS);
    }

    #[test]
    fn classify_bug_wins_over_insight() {
        let s = BuddySettings::default();
        let reaction =
            classify_chat_reaction("there is an error in the refactor plan", &s).unwrap();
        assert_eq!(reaction.kind, ChatReactionKind::BugCandidate);
    }

    #[test]
    fn classify_insight_wins_over_humor() {
        let s = BuddySettings::default();
        let reaction =
            classify_chat_reaction("let us design a new system architecture", &s).unwrap();
        assert_eq!(reaction.kind, ChatReactionKind::Insight);
    }

    #[test]
    fn classify_humor_fallback() {
        let s = BuddySettings::default();
        let reaction =
            classify_chat_reaction("please write a hello world example for me", &s).unwrap();
        assert_eq!(reaction.kind, ChatReactionKind::Humor);
    }

    #[test]
    fn classify_none_when_humor_off() {
        let mut s = BuddySettings::default();
        s.humor_level = HumorLevel::Off;
        assert!(classify_chat_reaction("please write a hello world example for me", &s).is_none());
    }

    #[test]
    fn reaction_event_metadata() {
        let s = BuddySettings::default();
        let analysis_text = "there is a crash in production today";
        let reaction = classify_chat_reaction(analysis_text, &s).unwrap();
        assert_eq!(reaction.kind, ChatReactionKind::BugCandidate);

        let ev = build_reaction_event("chat-1", analysis_text, &reaction);

        assert_eq!(ev.chat_id.as_deref(), Some("chat-1"));
        assert!(ev.speech_text.is_some());
        assert!(ev.ttl_ms.is_some());
        assert!(ev.ttl_ms.unwrap() > 0);
        assert_eq!(ev.bubble_policy, Some(BuddyBubblePolicy::EventOnce));
        assert_eq!(ev.signal_type, "chat_bug_candidate");

        let dedupe = ev.dedupe_key.unwrap();
        assert!(dedupe.contains("chat-1"));
        assert!(!dedupe.contains("crash"), "raw content must not appear in dedupe key");
    }

    #[test]
    fn reaction_event_no_secret_in_title_or_dedupe_key() {
        let raw = "connection failed: Bearer sk-VERYSECRET crashed";
        let analysis = prepare_analysis_text(raw).unwrap();
        let s = BuddySettings::default();
        let reaction = classify_chat_reaction(&analysis, &s).unwrap();
        let ev = build_reaction_event("chat-sec", &analysis, &reaction);

        assert!(!ev.title.contains("sk-VERYSECRET"), "secret must not appear in title");
        assert!(!ev.title.contains("VERYSECRET"), "secret must not appear in title");
        let dedupe = ev.dedupe_key.unwrap();
        assert!(!dedupe.contains("sk-VERYSECRET"), "secret must not appear in dedupe key");
        assert!(!dedupe.contains("VERYSECRET"), "secret must not appear in dedupe key");
    }

    #[test]
    fn limiter_per_chat_cooldown() {
        let mut lim = ChatReactionLimiter::new();
        let now = Utc::now();
        assert!(lim.allow("chat-a", now));
        assert!(!lim.allow("chat-a", now + Duration::seconds(10)));
        assert!(lim.allow("chat-a", now + Duration::seconds(PER_CHAT_COOLDOWN_SECS + 1)));
    }

    #[test]
    fn limiter_global_hourly_cap() {
        let mut lim = ChatReactionLimiter::new();
        let now = Utc::now();
        for i in 0..GLOBAL_HOURLY_CAP {
            let chat_id = format!("chat-{i}");
            assert!(lim.allow(&chat_id, now + Duration::seconds(i64::from(i) * 300)));
        }
        let overflow_chat = format!("chat-{}", GLOBAL_HOURLY_CAP);
        assert!(!lim.allow(
            &overflow_chat,
            now + Duration::seconds(i64::from(GLOBAL_HOURLY_CAP) * 300)
        ));
    }

    #[test]
    fn limiter_resets_after_hour() {
        let mut lim = ChatReactionLimiter::new();
        let now = Utc::now();
        for i in 0..GLOBAL_HOURLY_CAP {
            let chat_id = format!("chat-reset-{i}");
            lim.allow(&chat_id, now + Duration::seconds(i64::from(i) * 300));
        }
        let after_hour = now + Duration::seconds(3601);
        assert!(lim.allow("chat-fresh", after_hour));
    }
}
