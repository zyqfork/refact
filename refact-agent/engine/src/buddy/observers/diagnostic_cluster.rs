use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::buddy::diagnostics::DiagnosticContext;
use crate::buddy::observers::{BuddyObserver, ObserverContext};
use crate::buddy::settings::BuddySettings;
use crate::buddy::types::{BuddyFact, BuddyFactKind};
use crate::global_context::GlobalContext;

pub struct DiagnosticClusterObserver;

pub fn detect_diagnostic_cluster_facts(
    diagnostics: &[DiagnosticContext],
    now: DateTime<Utc>,
) -> Vec<BuddyFact> {
    let mut facts = vec![];
    let window_30min = now - chrono::Duration::minutes(30);
    let window_5min = now - chrono::Duration::minutes(5);

    let mut by_type: HashMap<&str, (u32, &str)> = HashMap::new();
    let mut fe_count: u32 = 0;
    let mut fe_sample: Option<&str> = None;

    for diag in diagnostics {
        let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&diag.collected_at) else {
            continue;
        };
        let ts_utc = ts.with_timezone(&Utc);

        if ts_utc >= window_30min {
            let entry = by_type
                .entry(diag.error_type.as_str())
                .or_insert((0, diag.collected_at.as_str()));
            entry.0 += 1;
        }

        if ts_utc >= window_5min {
            if diag.tool_name.as_deref() == Some("frontend") {
                fe_count += 1;
                if fe_sample.is_none() {
                    fe_sample = Some(diag.collected_at.as_str());
                }
            }
        }
    }

    for (error_type, (count, sample)) in &by_type {
        if *count >= 3 {
            tracing::debug!("diagnostic_cluster: type={} count={}", error_type, count);
            facts.push(BuddyFact {
                kind: BuddyFactKind::DiagnosticCluster,
                key: format!("diag:cluster:{}", error_type),
                source: "diagnostic_cluster",
                payload: serde_json::json!({
                    "error_type": error_type,
                    "count": count,
                    "window_seconds": 1800,
                    "sample_diagnostic_id": sample,
                }),
                seen_at: now,
                confidence: 0.9,
            });
        }
    }

    if fe_count >= 5 {
        tracing::debug!("diagnostic_cluster: frontend burst count={}", fe_count);
        facts.push(BuddyFact {
            kind: BuddyFactKind::FrontendErrorBurst,
            key: "diag:fe_burst:global".to_string(),
            source: "diagnostic_cluster",
            payload: serde_json::json!({
                "error_type": "frontend",
                "count": fe_count,
                "window_seconds": 300,
                "sample_diagnostic_id": fe_sample.unwrap_or(""),
            }),
            seen_at: now,
            confidence: 0.95,
        });
    }

    facts
}

#[async_trait::async_trait]
impl BuddyObserver for DiagnosticClusterObserver {
    fn id(&self) -> &'static str {
        "diagnostic_cluster"
    }

    fn cadence_seconds(&self) -> u64 {
        60
    }

    fn requires_setting(&self, settings: &BuddySettings) -> bool {
        settings.observers.diagnostic_cluster
    }

    async fn observe(
        &self,
        gcx: Arc<RwLock<GlobalContext>>,
        ctx: &ObserverContext,
    ) -> Vec<BuddyFact> {
        let buddy_arc = gcx.read().await.buddy.clone();
        let lock = buddy_arc.lock().await;
        let diagnostics = match lock.as_ref() {
            Some(svc) => svc.recent_diagnostics.clone(),
            None => return vec![],
        };
        drop(lock);
        detect_diagnostic_cluster_facts(&diagnostics, ctx.now)
    }
}
