use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn serialize_path<S: serde::Serializer>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&path.to_string_lossy())
}

fn deserialize_path<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<PathBuf, D::Error> {
    Ok(PathBuf::from(String::deserialize(deserializer)?))
}

fn default_enforce() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeMeta {
    pub id: String,
    pub kind: String,
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub root: PathBuf,
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub source_workspace_root: PathBuf,
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default = "default_enforce")]
    pub enforce: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_worktree_meta() -> WorktreeMeta {
        WorktreeMeta {
            id: "wt-1".to_string(),
            kind: "task_agent".to_string(),
            root: PathBuf::from("/tmp/refact-worktree"),
            source_workspace_root: PathBuf::from("/tmp/refact-source"),
            repo_root: PathBuf::from("/tmp/refact-source"),
            branch: Some("refact/task/t/card/a".to_string()),
            base_branch: Some("main".to_string()),
            base_commit: Some("abc123".to_string()),
            task_id: Some("task-1".to_string()),
            card_id: Some("card-1".to_string()),
            agent_id: Some("agent-1".to_string()),
            enforce: true,
        }
    }

    #[test]
    fn worktree_meta_serde_roundtrip() {
        let meta = sample_worktree_meta();
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["id"], "wt-1");
        assert_eq!(json["kind"], "task_agent");
        assert_eq!(json["root"], "/tmp/refact-worktree");
        assert_eq!(json["source_workspace_root"], "/tmp/refact-source");
        assert_eq!(json["repo_root"], "/tmp/refact-source");
        assert_eq!(json["enforce"], true);
        let roundtrip: WorktreeMeta = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, meta);
    }

    #[test]
    fn worktree_meta_enforce_defaults_false() {
        let json = serde_json::json!({
            "id": "wt-1",
            "kind": "manual",
            "root": "/tmp/wt",
            "source_workspace_root": "/tmp/src",
            "repo_root": "/tmp/src"
        });
        let meta: WorktreeMeta = serde_json::from_value(json).unwrap();
        assert!(!meta.enforce);
    }
}
