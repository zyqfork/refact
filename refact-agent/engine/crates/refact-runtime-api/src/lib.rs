use async_trait::async_trait;
use refact_buddy_core::snapshot::BuddySnapshot;
use refact_buddy_core::types::{BuddyRuntimeEvent, BuddySuggestion};
use refact_buddy_core::user_action::UserAction;
use refact_chat_api::ChatMessage;
use refact_chat_history::trajectory_snapshot::TrajectorySnapshot;
use refact_tool_api::ToolDesc;

pub use refact_chat_api::{SessionState, TaskMeta};
pub use refact_tool_api::ToolDesc as RuntimeToolDesc;
pub use refact_buddy_core::types::BuddyRuntimeEvent as RuntimeBuddyEvent;
pub use refact_buddy_core::user_action::UserAction as RuntimeUserAction;
pub use refact_chat_history::trajectory_snapshot::TrajectorySnapshot as RuntimeTrajectorySnapshot;

#[async_trait]
pub trait ActivitySink: Send + Sync {
    async fn record_user_action(&self, action: UserAction);
}

#[async_trait]
pub trait BuddyEventSink: Send + Sync {
    async fn enqueue_event(&self, event: BuddyRuntimeEvent);
    async fn complete_event(&self, dedupe_key: &str, status: &str);
    async fn snapshot(&self) -> Option<BuddySnapshot>;
    async fn apply_chat_completion(&self, event: BuddyRuntimeEvent, xp: u64, mood: String);
    async fn report_error(&self, error_type: &str, error_msg: &str, source: Option<&str>, chat_id: Option<&str>);
    async fn mark_chat_error(&self, event: BuddyRuntimeEvent);
    async fn maybe_add_suggestion(&self, suggestion: BuddySuggestion);
    async fn build_pulse_message(&self) -> Option<ChatMessage>;
    async fn render_runtime_event_fast(
        &self,
        workflow_id: &str,
        workflow_summary: &str,
        status: &str,
    ) -> Option<(String, Option<String>)>;
}

#[async_trait]
pub trait ToolRegistry: Send + Sync {
    async fn get_tools_for_mode(&self, mode: &str) -> Vec<ToolDesc>;
}

#[async_trait]
pub trait ChatSessionFacade: Send + Sync {
    async fn save_trajectory_snapshot(&self, snapshot: TrajectorySnapshot) -> Result<(), String>;
}
