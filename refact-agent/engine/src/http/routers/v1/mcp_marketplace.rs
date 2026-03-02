use std::collections::HashMap;
use std::sync::Arc;
use axum::Extension;
use axum::response::Json;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock as ARwLock;
use tokio::time::{Duration, Instant};
use std::sync::Mutex;

use crate::custom_error::ScratchError;
use crate::global_context::GlobalContext;

const REMOTE_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/smallcloudai/refact/refs/heads/main/refact-agent/engine/src/yaml_configs/mcp_marketplace_index.json";
const CACHE_TTL_SECS: u64 = 3600;
static INDEX_CACHE: Mutex<Option<(Instant, MarketplaceIndex)>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecipe {
    pub command: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceServer {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    pub transport: String,
    pub install_recipe: InstallRecipe,
    #[serde(default)]
    pub confirmation_default: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceIndex {
    pub version: u32,
    pub updated_at: String,
    pub servers: Vec<MarketplaceServer>,
}

fn bundled_index() -> MarketplaceIndex {
    serde_json::from_str(include_str!("../../../yaml_configs/mcp_marketplace_index.json"))
        .expect("bundled MCP marketplace index must be valid JSON")
}

async fn fetch_remote_index(http_client: &reqwest::Client) -> Option<MarketplaceIndex> {
    let resp = http_client
        .get(REMOTE_REGISTRY_URL)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<MarketplaceIndex>().await.ok()
}

fn merge_indices(remote: MarketplaceIndex, local: MarketplaceIndex) -> (MarketplaceIndex, &'static str) {
    let mut merged_map: indexmap::IndexMap<String, MarketplaceServer> = indexmap::IndexMap::new();
    for s in local.servers {
        merged_map.insert(s.id.clone(), s);
    }
    for s in remote.servers {
        merged_map.insert(s.id.clone(), s);
    }
    let servers: Vec<MarketplaceServer> = merged_map.into_values().collect();
    (
        MarketplaceIndex { version: 1, updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(), servers },
        "merged",
    )
}

async fn load_index(gcx: Arc<ARwLock<GlobalContext>>) -> (MarketplaceIndex, &'static str) {
    {
        let guard = INDEX_CACHE.lock().unwrap();
        if let Some((ts, ref idx)) = *guard {
            if ts.elapsed().as_secs() < CACHE_TTL_SECS {
                return (idx.clone(), "remote");
            }
        }
    }

    let http_client = gcx.read().await.http_client.clone();
    let local = bundled_index();

    match fetch_remote_index(&http_client).await {
        Some(remote) => {
            let (merged, source) = merge_indices(remote, local);
            let mut guard = INDEX_CACHE.lock().unwrap();
            *guard = Some((Instant::now(), merged.clone()));
            (merged, source)
        }
        None => {
            let mut guard = INDEX_CACHE.lock().unwrap();
            *guard = Some((Instant::now(), local.clone()));
            (local, "local")
        }
    }
}

pub async fn handle_v1_mcp_marketplace_get(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (index, source) = load_index(gcx).await;
    Ok(Json(json!({
        "servers": index.servers,
        "source": source,
    })))
}

#[derive(Deserialize)]
pub struct InstallRequest {
    pub server_id: String,
    #[serde(default)]
    pub config_overrides: Option<ConfigOverrides>,
}

#[derive(Deserialize, Default)]
pub struct ConfigOverrides {
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub async fn handle_v1_mcp_marketplace_install(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Json<Value>, ScratchError> {
    let req = serde_json::from_slice::<InstallRequest>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON: {}", e)))?;

    if req.server_id.is_empty()
        || req.server_id.contains('/')
        || req.server_id.contains('\\')
        || req.server_id.contains("..")
    {
        return Err(ScratchError::new(StatusCode::BAD_REQUEST, "invalid server_id".to_string()));
    }

    let (index, _) = load_index(gcx.clone()).await;
    let server = index
        .servers
        .iter()
        .find(|s| s.id == req.server_id)
        .ok_or_else(|| ScratchError::new(StatusCode::NOT_FOUND, format!("server '{}' not found in marketplace", req.server_id)))?;

    let config_dir = gcx.read().await.config_dir.clone();
    let integrations_dir = config_dir.join("integrations.d");
    tokio::fs::create_dir_all(&integrations_dir).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("cannot create integrations dir: {}", e)))?;

    let prefix = match server.transport.as_str() {
        "http" | "streamable-http" => "mcp_http",
        "sse" => "mcp_sse",
        _ => "mcp_stdio",
    };
    let safe_id = req.server_id.replace('-', "_");
    let filename = format!("{}_{}.yaml", prefix, safe_id);
    let config_path = integrations_dir.join(&filename);

    if config_path.exists() {
        return Err(ScratchError::new(StatusCode::CONFLICT, format!("config file '{}' already exists", filename)));
    }

    let mut env = server.install_recipe.env.clone();
    if let Some(overrides) = &req.config_overrides {
        for (k, v) in &overrides.env {
            env.insert(k.clone(), v.clone());
        }
    }

    let yaml_content = build_integration_yaml(server, &env);
    let tmp_path = config_path.with_extension("yaml.tmp");
    tokio::fs::write(&tmp_path, &yaml_content).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("write error: {}", e)))?;
    tokio::fs::rename(&tmp_path, &config_path).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("rename error: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "config_path": config_path.display().to_string(),
    })))
}

fn build_integration_yaml(server: &MarketplaceServer, env: &HashMap<String, String>) -> String {
    let mut lines = vec![
        format!("command: {:?}", server.install_recipe.command),
        "env:".to_string(),
    ];
    if env.is_empty() {
        lines.push("  {}".to_string());
    } else {
        for (k, v) in env {
            lines.push(format!("  {}: {:?}", k, v));
        }
    }
    lines.push("init_timeout: \"60\"".to_string());
    lines.push("request_timeout: \"30\"".to_string());
    lines.push("available:".to_string());
    lines.push("  on_your_laptop: true".to_string());
    lines.push("  when_isolated: false".to_string());
    lines.push("confirmation:".to_string());
    if server.confirmation_default.is_empty() {
        lines.push("  ask_user_default: []".to_string());
    } else {
        let items: Vec<String> = server.confirmation_default.iter().map(|s| format!("\"{}\"", s)).collect();
        lines.push(format!("  ask_user_default: [{}]", items.join(", ")));
    }
    lines.join("\n") + "\n"
}

pub async fn handle_v1_mcp_marketplace_installed(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (index, _) = load_index(gcx.clone()).await;
    let config_dir = gcx.read().await.config_dir.clone();
    let integrations_dir = config_dir.join("integrations.d");

    let index_ids: std::collections::HashSet<String> = index.servers.iter().map(|s| s.id.clone()).collect();
    let mut installed = Vec::new();

    let read_dir = match tokio::fs::read_dir(&integrations_dir).await {
        Ok(rd) => rd,
        Err(_) => {
            return Ok(Json(json!({ "installed": installed })));
        }
    };

    let mut rd = read_dir;
    while let Ok(Some(entry)) = rd.next_entry().await {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if !fname_str.ends_with(".yaml") {
            continue;
        }
        for prefix in &["mcp_stdio_", "mcp_sse_", "mcp_http_"] {
            if let Some(rest) = fname_str.strip_prefix(prefix) {
                let id_candidate = rest.trim_end_matches(".yaml").replace('_', "-");
                if index_ids.contains(&id_candidate) {
                    let server = index.servers.iter().find(|s| s.id == id_candidate).unwrap();
                    installed.push(json!({
                        "id": id_candidate,
                        "name": server.name,
                        "config_path": entry.path().display().to_string(),
                    }));
                }
                break;
            }
        }
    }

    Ok(Json(json!({ "installed": installed })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_index_parses() {
        let index = bundled_index();
        assert!(index.version >= 1, "version must be >= 1");
        assert!(index.servers.len() >= 10, "must have at least 10 servers, got {}", index.servers.len());
    }

    #[test]
    fn test_bundled_index_all_servers_have_required_fields() {
        let index = bundled_index();
        for server in &index.servers {
            assert!(!server.id.is_empty(), "server id must not be empty");
            assert!(!server.name.is_empty(), "server name must not be empty for id={}", server.id);
            assert!(!server.description.is_empty(), "server description must not be empty for id={}", server.id);
            assert!(!server.transport.is_empty(), "server transport must not be empty for id={}", server.id);
            assert!(!server.install_recipe.command.is_empty(), "install command must not be empty for id={}", server.id);
        }
    }

    #[test]
    fn test_bundled_index_no_duplicate_ids() {
        let index = bundled_index();
        let mut ids = std::collections::HashSet::new();
        for server in &index.servers {
            assert!(ids.insert(server.id.clone()), "duplicate server id: {}", server.id);
        }
    }

    #[test]
    fn test_build_integration_yaml_stdio_with_env() {
        let server = MarketplaceServer {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            description: "GitHub MCP server".to_string(),
            publisher: "github".to_string(),
            tags: vec!["vcs".to_string()],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe {
                command: "npx -y @modelcontextprotocol/server-github".to_string(),
                env: HashMap::new(),
            },
            confirmation_default: vec!["*".to_string()],
        };
        let mut env = HashMap::new();
        env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), "ghp_test".to_string());
        let yaml = build_integration_yaml(&server, &env);
        assert!(yaml.contains("npx -y @modelcontextprotocol/server-github"), "yaml must contain command");
        assert!(yaml.contains("GITHUB_PERSONAL_ACCESS_TOKEN"), "yaml must contain env key");
        assert!(yaml.contains("ghp_test"), "yaml must contain env value");
        assert!(yaml.contains("init_timeout"), "yaml must contain init_timeout");
        assert!(yaml.contains("request_timeout"), "yaml must contain request_timeout");
        assert!(yaml.contains("ask_user_default"), "yaml must contain confirmation");
    }

    #[test]
    fn test_build_integration_yaml_empty_env() {
        let server = MarketplaceServer {
            id: "fetch".to_string(),
            name: "Fetch".to_string(),
            description: "Fetch server".to_string(),
            publisher: "anthropic".to_string(),
            tags: vec![],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe {
                command: "uvx mcp-server-fetch".to_string(),
                env: HashMap::new(),
            },
            confirmation_default: vec!["*".to_string()],
        };
        let yaml = build_integration_yaml(&server, &HashMap::new());
        assert!(yaml.contains("env:"), "yaml must contain env section");
        assert!(yaml.contains("{}"), "yaml must show empty env as {{}}"); 
    }

    #[test]
    fn test_merge_indices_remote_overrides_local() {
        let local_server = MarketplaceServer {
            id: "github".to_string(),
            name: "GitHub Local".to_string(),
            description: "old description".to_string(),
            publisher: "local".to_string(),
            tags: vec![],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe { command: "old-command".to_string(), env: HashMap::new() },
            confirmation_default: vec![],
        };
        let remote_server = MarketplaceServer {
            id: "github".to_string(),
            name: "GitHub Remote".to_string(),
            description: "new description".to_string(),
            publisher: "github".to_string(),
            tags: vec![],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe { command: "new-command".to_string(), env: HashMap::new() },
            confirmation_default: vec!["*".to_string()],
        };
        let local_idx = MarketplaceIndex { version: 1, updated_at: "2026-01-01".to_string(), servers: vec![local_server] };
        let remote_idx = MarketplaceIndex { version: 1, updated_at: "2026-02-01".to_string(), servers: vec![remote_server] };
        let (merged, _) = merge_indices(remote_idx, local_idx);
        assert_eq!(merged.servers.len(), 1, "should merge to one server");
        assert_eq!(merged.servers[0].name, "GitHub Remote", "remote should override local");
        assert_eq!(merged.servers[0].install_recipe.command, "new-command");
    }

    #[test]
    fn test_merge_indices_keeps_local_only_servers() {
        let local_only = MarketplaceServer {
            id: "custom-local".to_string(),
            name: "Custom Local".to_string(),
            description: "local only".to_string(),
            publisher: "local".to_string(),
            tags: vec![],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe { command: "custom-cmd".to_string(), env: HashMap::new() },
            confirmation_default: vec![],
        };
        let remote_server = MarketplaceServer {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            description: "github".to_string(),
            publisher: "github".to_string(),
            tags: vec![],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe { command: "npx github".to_string(), env: HashMap::new() },
            confirmation_default: vec!["*".to_string()],
        };
        let local_idx = MarketplaceIndex { version: 1, updated_at: "2026-01-01".to_string(), servers: vec![local_only] };
        let remote_idx = MarketplaceIndex { version: 1, updated_at: "2026-02-01".to_string(), servers: vec![remote_server] };
        let (merged, _) = merge_indices(remote_idx, local_idx);
        assert_eq!(merged.servers.len(), 2, "merged should have both servers");
        assert!(merged.servers.iter().any(|s| s.id == "custom-local"), "must contain local-only server");
        assert!(merged.servers.iter().any(|s| s.id == "github"), "must contain remote server");
    }

    #[tokio::test]
    async fn test_install_creates_correct_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let integrations_dir = tmp.path().join("integrations.d");
        tokio::fs::create_dir_all(&integrations_dir).await.unwrap();

        let server = MarketplaceServer {
            id: "brave-search".to_string(),
            name: "Brave Search".to_string(),
            description: "Web search".to_string(),
            publisher: "anthropic".to_string(),
            tags: vec!["search".to_string()],
            icon_url: None,
            homepage: None,
            transport: "stdio".to_string(),
            install_recipe: InstallRecipe {
                command: "npx -y @modelcontextprotocol/server-brave-search".to_string(),
                env: { let mut m = HashMap::new(); m.insert("BRAVE_API_KEY".to_string(), "".to_string()); m },
            },
            confirmation_default: vec!["*".to_string()],
        };

        let mut env = server.install_recipe.env.clone();
        env.insert("BRAVE_API_KEY".to_string(), "test-key-123".to_string());

        let yaml = build_integration_yaml(&server, &env);
        let config_path = integrations_dir.join("mcp_stdio_brave_search.yaml");
        tokio::fs::write(&config_path, &yaml).await.unwrap();

        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        assert!(content.contains("npx -y @modelcontextprotocol/server-brave-search"));
        assert!(content.contains("BRAVE_API_KEY"));
        assert!(content.contains("test-key-123"));
        assert!(content.contains("init_timeout"));
        assert!(content.contains("request_timeout"));
        assert!(content.contains("ask_user_default"));
    }

    #[tokio::test]
    async fn test_installed_detection() {
        let tmp = tempfile::tempdir().unwrap();
        let integrations_dir = tmp.path().join("integrations.d");
        tokio::fs::create_dir_all(&integrations_dir).await.unwrap();
        tokio::fs::write(integrations_dir.join("mcp_stdio_github.yaml"), "command: npx github\n").await.unwrap();
        tokio::fs::write(integrations_dir.join("mcp_stdio_brave_search.yaml"), "command: npx brave\n").await.unwrap();
        tokio::fs::write(integrations_dir.join("other_integration.yaml"), "some: config\n").await.unwrap();

        let index = bundled_index();
        let index_ids: std::collections::HashSet<String> = index.servers.iter().map(|s| s.id.clone()).collect();

        let mut installed_ids = Vec::new();
        let mut rd = tokio::fs::read_dir(&integrations_dir).await.unwrap();
        while let Ok(Some(entry)) = rd.next_entry().await {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy().to_string();
            if !fname_str.ends_with(".yaml") { continue; }
            for prefix in &["mcp_stdio_", "mcp_sse_", "mcp_http_"] {
                if let Some(rest) = fname_str.strip_prefix(prefix) {
                    let id_candidate = rest.trim_end_matches(".yaml").replace('_', "-");
                    if index_ids.contains(&id_candidate) {
                        installed_ids.push(id_candidate);
                    }
                    break;
                }
            }
        }
        assert!(installed_ids.contains(&"github".to_string()), "must detect github as installed");
        assert!(installed_ids.contains(&"brave-search".to_string()), "must detect brave-search as installed");
        assert!(!installed_ids.contains(&"other".to_string()), "must not detect non-mcp integrations");
    }
}
