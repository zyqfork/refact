use std::sync::Arc;

use super::super::actor::make_runtime_event;
use super::super::scheduler::{BuddyJob, BuddyJobContext, BuddyJobResult};
use super::super::types::{BuddyActivity, BuddyRuntimeEvent, BuddySuggestion};

pub struct HealthWatcherJob;

#[async_trait::async_trait]
impl BuddyJob for HealthWatcherJob {
    fn id(&self) -> &str {
        "health_watcher"
    }
    fn cooldown_seconds(&self) -> u64 {
        900
    }
    fn priority(&self) -> u32 {
        4
    }
    fn produces_suggestion(&self) -> bool {
        true
    }

    async fn should_run(
        &self,
        _gcx: Arc<tokio::sync::RwLock<crate::global_context::GlobalContext>>,
        _ctx: &BuddyJobContext,
    ) -> bool {
        true
    }

    async fn execute(
        &self,
        gcx: Arc<tokio::sync::RwLock<crate::global_context::GlobalContext>>,
        ctx: BuddyJobContext,
    ) -> BuddyJobResult {
        let caps_result = {
            let gcx_locked = gcx.read().await;
            gcx_locked
                .caps
                .as_ref()
                .map(|c| (!c.completion_models.is_empty(), !c.chat_models.is_empty()))
        };

        let Some((has_completion, has_chat)) = caps_result else {
            return BuddyJobResult::default();
        };

        let was_healthy = ctx.job_state.last_result.as_deref() == Some("healthy");

        if !has_completion && !has_chat {
            let suggestion = if !was_healthy {
                None
            } else {
                Some(BuddySuggestion {
                    id: format!("health-no-models-{}", chrono::Utc::now().timestamp()),
                    suggestion_type: "health".to_string(),
                    title: "No AI models configured".to_string(),
                    description: "I couldn't find any completion or chat models. Head to Provider Settings to add one.".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    dismissed: false,
                    controls: vec![],
                    quest: None,
                })
            };
            let event: BuddyRuntimeEvent = make_runtime_event(
                "health",
                "No AI models configured",
                "health_watcher",
                "health_models",
                "failed",
                Some("high"),
            );
            return BuddyJobResult {
                suggestion,
                runtime_event: Some(event),
                last_result: Some("unhealthy".to_string()),
                ..Default::default()
            };
        }

        let parts: Vec<&str> = [
            if has_completion {
                Some("completion")
            } else {
                None
            },
            if has_chat { Some("chat") } else { None },
        ]
        .into_iter()
        .flatten()
        .collect();
        let title = format!("Models ready: {}", parts.join(", "));

        let activity = if !was_healthy {
            Some(BuddyActivity {
                icon: "💚".to_string(),
                title: title.clone(),
                description: format!("Health check passed — {}", title),
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "health".to_string(),
            })
        } else {
            None
        };

        let event: BuddyRuntimeEvent = make_runtime_event(
            "health",
            &title,
            "health_watcher",
            "health_models",
            "completed",
            None,
        );
        BuddyJobResult {
            activity,
            runtime_event: Some(event),
            last_result: Some("healthy".to_string()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_watcher_declares_suggestion_output() {
        let job = HealthWatcherJob;

        assert!(job.produces_suggestion());
    }
}
