use std::sync::Arc;
use super::super::scheduler::{BuddyJob, BuddyJobContext, BuddyJobResult};
use super::super::types::BuddySuggestion;

pub struct ProactiveSuggestionsJob;

#[async_trait::async_trait]
impl BuddyJob for ProactiveSuggestionsJob {
    fn id(&self) -> &str {
        "proactive_suggestions"
    }
    fn cooldown_seconds(&self) -> u64 {
        3600
    }
    fn priority(&self) -> u32 {
        6
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
        _gcx: Arc<tokio::sync::RwLock<crate::global_context::GlobalContext>>,
        ctx: BuddyJobContext,
    ) -> BuddyJobResult {
        if ctx.job_state.run_count == 0 {
            return BuddyJobResult::default();
        }

        let project_root = ctx.project_root.clone();
        let file_count =
            tokio::task::spawn_blocking(move || count_uncommitted_changes(&project_root))
                .await
                .unwrap_or(None);

        let Some(count) = file_count else {
            return BuddyJobResult::default();
        };

        if count >= 10 {
            return BuddyJobResult {
                suggestion: Some(BuddySuggestion {
                    id: format!("git-uncommitted-{}", chrono::Utc::now().timestamp()),
                    suggestion_type: "git_commit".to_string(),
                    title: format!("{} uncommitted files", count),
                    description:
                        "You have many uncommitted changes. Want me to generate a commit message?"
                            .to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    dismissed: false,
                }),
                ..Default::default()
            };
        }

        BuddyJobResult::default()
    }
}

fn count_uncommitted_changes(project_root: &std::path::Path) -> Option<usize> {
    use git2::{Repository, StatusOptions, StatusShow};
    let repo = Repository::open(project_root).ok()?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .show(StatusShow::IndexAndWorkdir);
    let statuses = repo.statuses(Some(&mut opts)).ok()?;
    Some(statuses.iter().filter(|s| !s.status().is_empty()).count())
}
