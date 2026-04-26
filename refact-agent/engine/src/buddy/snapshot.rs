use serde::{Deserialize, Serialize};
use super::settings::BuddySettings;
use super::types::BuddyState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuddySnapshot {
    pub state: BuddyState,
    pub settings: BuddySettings,
    pub enabled: bool,
}
