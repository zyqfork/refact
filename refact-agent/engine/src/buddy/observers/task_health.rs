use std::collections::HashSet;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::buddy::observers::{BuddyObserver, ObserverContext, ObserverCost};
use crate::buddy::settings::BuddySettings;
use crate::buddy::types::{BuddyFact, BuddyFactKind};
use crate::global_context::GlobalContext;
use crate::tasks::types::{TaskBoard, TaskMeta, TaskStatus};

pub struct TaskHealthObserver;

fn title_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:x}", h.finish())
}

pub fn detect_task_health_facts(
    pairs: &[(TaskMeta, TaskBoard)],
    now: DateTime<Utc>,
) -> Vec<BuddyFact> {
    let mut facts = vec![];
    let stuck_min = chrono::Duration::minutes(15);
    let abandon_days = chrono::Duration::days(7);

    for (meta, board) in pairs {
        let terminal = matches!(meta.status, TaskStatus::Completed | TaskStatus::Abandoned);
        if terminal {
            continue;
        }

        for card in &board.cards {
            if card.column != "doing" || card.assignee.is_none() {
                continue;
            }
            if let Some(started) = &card.started_at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(started) {
                    if now.signed_duration_since(dt.with_timezone(&Utc)) >= stuck_min {
                        tracing::debug!("task_health: stuck card {} in task {}", card.id, meta.id);
                        facts.push(BuddyFact {
                            kind: BuddyFactKind::TaskStuck,
                            key: format!("task:stuck:{}", meta.id),
                            source: "task_health",
                            payload: serde_json::json!({
                                "task_id": meta.id,
                                "card_id": card.id,
                                "last_seen_iso": started,
                                "agent_id": card.assignee,
                                "blocker_hint": "",
                            }),
                            seen_at: now,
                            confidence: 0.8,
                        });
                        break;
                    }
                }
            }
        }

        if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&meta.created_at) {
            let age = now.signed_duration_since(created.with_timezone(&Utc));
            let no_activity = board.cards.iter().all(|c| c.started_at.is_none());
            if age >= abandon_days && no_activity {
                tracing::debug!("task_health: abandoned task {}", meta.id);
                facts.push(BuddyFact {
                    kind: BuddyFactKind::TaskAbandoned,
                    key: format!("task:abandoned:{}", meta.id),
                    source: "task_health",
                    payload: serde_json::json!({
                        "task_id": meta.id,
                        "age_days": age.num_days(),
                    }),
                    seen_at: now,
                    confidence: 0.6,
                });
            }
        }
    }

    let active: Vec<&(TaskMeta, TaskBoard)> = pairs
        .iter()
        .filter(|(m, _)| !matches!(m.status, TaskStatus::Completed | TaskStatus::Abandoned))
        .collect();

    let mut emitted: HashSet<String> = HashSet::new();
    for i in 0..active.len() {
        for j in (i + 1)..active.len() {
            let a = active[i].0.name.to_lowercase();
            let a = a.trim();
            let b = active[j].0.name.to_lowercase();
            let b = b.trim();
            if a.is_empty() || b.is_empty() {
                continue;
            }
            let sim = strsim::normalized_levenshtein(a, b);
            if sim > 0.7 {
                let rep = if a <= b { a } else { b };
                let key = format!("task_cluster:{}", title_hash(rep));
                if emitted.insert(key.clone()) {
                    tracing::debug!("task_health: cluster {} ~ {}", active[i].0.id, active[j].0.id);
                    facts.push(BuddyFact {
                        kind: BuddyFactKind::TaskClusterDuplicate,
                        key,
                        source: "task_health",
                        payload: serde_json::json!({
                            "task_ids": [active[i].0.id, active[j].0.id],
                            "similarity": sim,
                        }),
                        seen_at: now,
                        confidence: 0.9,
                    });
                }
            }
        }
    }

    facts
}

#[async_trait::async_trait]
impl BuddyObserver for TaskHealthObserver {
    fn id(&self) -> &'static str {
        "task_health"
    }

    fn cadence_seconds(&self) -> u64 {
        60
    }

    fn cost_class(&self) -> ObserverCost {
        ObserverCost::Io
    }

    fn requires_setting(&self, settings: &BuddySettings) -> bool {
        settings.observers.task_health
    }

    async fn observe(
        &self,
        gcx: Arc<RwLock<GlobalContext>>,
        ctx: &ObserverContext,
    ) -> Vec<BuddyFact> {
        let tasks = match crate::tasks::storage::list_tasks(gcx.clone()).await {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let mut pairs = vec![];
        for meta in tasks {
            match crate::tasks::storage::load_board(gcx.clone(), &meta.id).await {
                Ok(board) => pairs.push((meta, board)),
                Err(_) => continue,
            }
        }
        detect_task_health_facts(&pairs, ctx.now)
    }
}
