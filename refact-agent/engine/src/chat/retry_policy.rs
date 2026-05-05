use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    Retry { reason: &'static str },
    DoNotRetry { reason: &'static str },
}

pub const MAX_LLM_RETRY_ATTEMPTS: usize = 5;
pub const LLM_RETRY_DELAYS: [Duration; MAX_LLM_RETRY_ATTEMPTS] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(45),
    Duration::from_secs(120),
    Duration::from_secs(300),
];

const NON_RETRYABLE_STATUS_CODES: [&str; 10] = [
    "400", "401", "403", "404", "408", "409", "422", "423", "424", "426",
];

const RETRYABLE_STATUS_CODES: [&str; 8] = ["408", "425", "429", "500", "502", "503", "504", "529"];

const NON_RETRYABLE_PATTERNS: [&str; 36] = [
    "aborted",
    "cancelled",
    "canceled",
    "context window",
    "context length",
    "context_length",
    "maximum context",
    "too many tokens",
    "token limit",
    "prompt is too long",
    "input is too long",
    "invalid request",
    "bad request",
    "malformed",
    "schema",
    "json problem",
    "serialize",
    "deserialize",
    "authentication",
    "unauthorized",
    "invalid api key",
    "invalid key",
    "api key",
    "forbidden",
    "permission denied",
    "not found",
    "unsupported",
    "does not support",
    "safety",
    "policy",
    "content filter",
    "billing",
    "insufficient credits",
    "insufficient quota",
    "quota exceeded",
    "spend_limit_exceeded",
];

const RETRYABLE_PATTERNS: [&str; 25] = [
    "network",
    "timeout",
    "timed out",
    "connection",
    "connect",
    "dns",
    "eof",
    "broken pipe",
    "connection reset",
    "connection closed",
    "connection refused",
    "temporarily unavailable",
    "try again",
    "rate limit",
    "too many requests",
    "resource exhausted",
    "throttl",
    "overloaded",
    "overload",
    "capacity",
    "server error",
    "service unavailable",
    "bad gateway",
    "gateway timeout",
    "stream ended unexpectedly",
];

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn contains_retryable_status(lower: &str) -> bool {
    RETRYABLE_STATUS_CODES.iter().any(|code| {
        lower.contains(&format!("http {code}"))
            || lower.contains(&format!("status {code}"))
            || lower.contains(&format!("({code}"))
            || lower.contains(&format!("{code} "))
            || lower.ends_with(code)
    })
}

fn contains_non_retryable_status(lower: &str) -> bool {
    NON_RETRYABLE_STATUS_CODES.iter().any(|code| {
        lower.contains(&format!("http {code}"))
            || lower.contains(&format!("status {code}"))
            || lower.contains(&format!("({code}"))
            || lower.contains(&format!("{code} "))
            || lower.ends_with(code)
    })
}

pub fn classify_llm_error_for_retry(error: &str) -> RetryDecision {
    let lower = error.to_lowercase();

    if contains_any(&lower, &["aborted", "cancelled", "canceled"]) {
        return RetryDecision::DoNotRetry {
            reason: "cancelled",
        };
    }

    if contains_retryable_status(&lower) {
        return RetryDecision::Retry {
            reason: "retryable_http_status",
        };
    }

    if contains_non_retryable_status(&lower) || contains_any(&lower, &NON_RETRYABLE_PATTERNS) {
        return RetryDecision::DoNotRetry {
            reason: "non_retryable_error",
        };
    }

    if contains_any(&lower, &RETRYABLE_PATTERNS) {
        return RetryDecision::Retry {
            reason: "transient_error",
        };
    }

    RetryDecision::DoNotRetry {
        reason: "unknown_error",
    }
}

pub fn should_retry_llm_error(error: &str, retry_attempt: usize, abort_flag: &AtomicBool) -> bool {
    matches!(
        classify_llm_error_for_retry(error),
        RetryDecision::Retry { .. }
    ) && retry_attempt < MAX_LLM_RETRY_ATTEMPTS
        && !abort_flag.load(Ordering::SeqCst)
}

pub fn retry_delay_for_attempt(retry_attempt: usize) -> Duration {
    LLM_RETRY_DELAYS
        .get(retry_attempt)
        .copied()
        .unwrap_or_else(|| *LLM_RETRY_DELAYS.last().unwrap())
}

pub async fn sleep_or_abort(delay: Duration, abort_flag: Arc<AtomicBool>) -> bool {
    let mut heartbeat = tokio::time::interval(Duration::from_millis(200));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let sleep = tokio::time::sleep(delay);
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            _ = &mut sleep => return false,
            _ = heartbeat.tick() => {
                if abort_flag.load(Ordering::SeqCst) {
                    return true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_transient_http_statuses() {
        assert!(matches!(
            classify_llm_error_for_retry("LLM error (429 Too Many Requests): rate limit"),
            RetryDecision::Retry {
                reason: "retryable_http_status"
            }
        ));
        assert!(matches!(
            classify_llm_error_for_retry("LLM error (503 Service Unavailable): overloaded"),
            RetryDecision::Retry {
                reason: "retryable_http_status"
            }
        ));
    }

    #[test]
    fn retries_network_and_stream_transients() {
        assert!(matches!(
            classify_llm_error_for_retry("Stream error: connection reset by peer"),
            RetryDecision::Retry {
                reason: "transient_error"
            }
        ));
        assert!(matches!(
            classify_llm_error_for_retry("LLM stream ended unexpectedly without completion signal"),
            RetryDecision::Retry {
                reason: "transient_error"
            }
        ));
    }

    #[test]
    fn does_not_retry_auth_or_bad_request() {
        assert!(matches!(
            classify_llm_error_for_retry("LLM error (401 Unauthorized): invalid api key"),
            RetryDecision::DoNotRetry { .. }
        ));
        assert!(matches!(
            classify_llm_error_for_retry("LLM error (400 Bad Request): context length exceeded"),
            RetryDecision::DoNotRetry { .. }
        ));
    }

    #[test]
    fn retryable_status_wins_over_generic_validation_words() {
        assert!(matches!(
            classify_llm_error_for_retry("LLM error (429): ValidationException: rate limited"),
            RetryDecision::Retry { .. }
        ));
    }

    #[test]
    fn does_not_retry_user_cancellation() {
        assert!(matches!(
            classify_llm_error_for_retry("Aborted"),
            RetryDecision::DoNotRetry {
                reason: "cancelled"
            }
        ));
    }

    #[test]
    fn should_retry_obeys_attempt_limit_and_abort() {
        let flag = AtomicBool::new(false);
        assert!(should_retry_llm_error("timeout", 0, &flag));
        assert!(!should_retry_llm_error(
            "timeout",
            MAX_LLM_RETRY_ATTEMPTS,
            &flag
        ));
        flag.store(true, Ordering::SeqCst);
        assert!(!should_retry_llm_error("timeout", 0, &flag));
    }
}
