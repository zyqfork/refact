use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::buddy::observers::{BuddyObserver, ObserverContext, ObserverCost};
use crate::buddy::settings::BuddySettings;
use crate::buddy::types::{BuddyFact, BuddyFactKind};
use crate::global_context::GlobalContext;

pub struct TrajectoryClutterObserver;

fn path_hash(p: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    p.hash(&mut h);
    format!("{:x}", h.finish())
}

pub async fn scan_trajectories_dir(dir: &std::path::Path) -> (u32, u32, u32) {
    let mut total: u32 = 0;
    let mut untitled: u32 = 0;
    let mut oldest_age_days: u32 = 0;
    let now = Utc::now();

    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(r) => r,
        Err(_) => return (0, 0, 0),
    };

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            total += 1;
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let title = v
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if title.is_empty() {
                        untitled += 1;
                    }
                    if let Some(created) = v
                        .get("created_at")
                        .and_then(|t| t.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    {
                        let age = now
                            .signed_duration_since(created.with_timezone(&Utc))
                            .num_days()
                            .max(0) as u32;
                        if age > oldest_age_days {
                            oldest_age_days = age;
                        }
                    }
                } else {
                    untitled += 1;
                }
            }
        }
    }

    (total, untitled, oldest_age_days)
}

pub fn detect_trajectory_clutter_facts(
    project_root_hash: &str,
    total: u32,
    untitled: u32,
    oldest_age_days: u32,
    now: DateTime<Utc>,
) -> Vec<BuddyFact> {
    if total <= 50 && untitled <= 15 {
        return vec![];
    }
    tracing::debug!("trajectory_clutter: total={} untitled={}", total, untitled);
    vec![BuddyFact {
        kind: BuddyFactKind::TrajectoryClutter,
        key: format!("trajectory:clutter:{}", project_root_hash),
        source: "trajectory_clutter",
        payload: serde_json::json!({
            "count": total,
            "untitled_count": untitled,
            "oldest_age_days": oldest_age_days,
        }),
        seen_at: now,
        confidence: 0.9,
    }]
}

#[async_trait::async_trait]
impl BuddyObserver for TrajectoryClutterObserver {
    fn id(&self) -> &'static str {
        "trajectory_clutter"
    }

    fn cadence_seconds(&self) -> u64 {
        300
    }

    fn cost_class(&self) -> ObserverCost {
        ObserverCost::Io
    }

    fn requires_setting(&self, settings: &BuddySettings) -> bool {
        settings.observers.trajectory_clutter
    }

    async fn observe(
        &self,
        gcx: Arc<RwLock<GlobalContext>>,
        ctx: &ObserverContext,
    ) -> Vec<BuddyFact> {
        let traj_dir = ctx.project_root.join(".refact").join("trajectories");
        if !traj_dir.exists() {
            return vec![];
        }
        let hash = path_hash(&ctx.project_root);
        let (total, untitled, oldest) = scan_trajectories_dir(&traj_dir).await;
        let _ = gcx;
        detect_trajectory_clutter_facts(&hash, total, untitled, oldest, ctx.now)
    }
}
