use std::path::Path;
use serde::{Serialize, Deserialize};
use tokio::fs;
use tracing::warn;

pub const MAX_PALETTE_INDEX: usize = 7;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumorLevel {
    Off,
    Light,
    Normal,
}

impl Default for HumorLevel {
    fn default() -> Self {
        Self::Light
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    ReadOnly,
    Suggest,
    SafeAuto,
}

impl Default for AutonomyLevel {
    fn default() -> Self {
        Self::Suggest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverToggles {
    #[serde(default = "default_true")]
    pub task_health: bool,
    #[serde(default = "default_true")]
    pub trajectory_clutter: bool,
    #[serde(default)]
    pub chat_pattern: bool,
    #[serde(default = "default_true")]
    pub customization_drift: bool,
    #[serde(default = "default_true")]
    pub memory_garden: bool,
    #[serde(default = "default_true")]
    pub mcp_auth: bool,
    #[serde(default = "default_true")]
    pub git_pressure: bool,
    #[serde(default = "default_true")]
    pub diagnostic_cluster: bool,
    #[serde(default = "default_true")]
    pub provider_health: bool,
}

impl Default for ObserverToggles {
    fn default() -> Self {
        Self {
            task_health: true,
            trajectory_clutter: true,
            chat_pattern: false,
            customization_drift: true,
            memory_garden: true,
            mcp_auth: true,
            git_pressure: true,
            diagnostic_cluster: true,
            provider_health: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuddySettings {
    pub enabled: bool,
    pub auto_diagnostics: bool,
    pub auto_issue_creation: bool,
    pub personality_prompt: Option<String>,
    #[serde(default = "default_true")]
    pub proactive_enabled: bool,
    #[serde(default)]
    pub message_observation_enabled: bool,
    #[serde(default = "default_true")]
    pub housekeeping_enabled: bool,
    #[serde(default = "default_true")]
    pub humor_enabled: bool,
    #[serde(default)]
    pub humor_level: HumorLevel,
    #[serde(default)]
    pub autonomy_level: AutonomyLevel,
    #[serde(default)]
    pub quiet_mode: bool,
    #[serde(default)]
    pub observers: ObserverToggles,
}

fn default_true() -> bool {
    true
}

impl Default for BuddySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_diagnostics: true,
            auto_issue_creation: false,
            personality_prompt: None,
            proactive_enabled: true,
            message_observation_enabled: false,
            housekeeping_enabled: true,
            humor_enabled: true,
            humor_level: HumorLevel::default(),
            autonomy_level: AutonomyLevel::default(),
            quiet_mode: false,
            observers: ObserverToggles::default(),
        }
    }
}

pub async fn load_settings(project_root: &Path) -> BuddySettings {
    let path = project_root.join(".refact/buddy/settings.json");
    match fs::read_to_string(&path).await {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to parse buddy settings: {}, using defaults", e);
                BuddySettings::default()
            }
        },
        Err(_) => BuddySettings::default(),
    }
}

pub async fn save_settings(project_root: &Path, settings: &BuddySettings) -> Result<(), String> {
    let path = project_root.join(".refact/buddy/settings.json");
    super::storage::atomic_write_json(&path, settings).await
}
