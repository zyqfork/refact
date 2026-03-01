use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as ARwLock;

use crate::global_context::GlobalContext;

const PLUGIN_SIZE_LIMIT: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketplaceOwner {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePluginEntry {
    pub name: String,
    #[serde(default)]
    pub source: serde_json::Value,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketplaceJson {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: Option<MarketplaceOwner>,
    #[serde(default)]
    pub plugins: Vec<MarketplacePluginEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    pub name: String,
    pub source: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPluginEntry {
    pub name: String,
    pub marketplace: String,
    #[serde(default)]
    pub version: String,
    pub install_dir: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsDb {
    #[serde(default)]
    pub marketplaces: Vec<MarketplaceEntry>,
    #[serde(default)]
    pub installed: Vec<InstalledPluginEntry>,
}

pub fn validate_plugin_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("plugin name cannot be empty".to_string());
    }
    if name.starts_with('.') {
        return Err("plugin name cannot start with '.'".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("plugin name contains invalid path characters".to_string());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        return Err("plugin name must match [a-zA-Z0-9._-]+".to_string());
    }
    Ok(())
}

fn validate_github_source(source: &str) -> Result<(), String> {
    let valid = source.split('/').count() == 2
        && source.split('/').all(|part| {
            !part.is_empty()
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
        });
    if valid {
        Ok(())
    } else {
        Err(format!("invalid GitHub source format '{}': must be 'owner/repo'", source))
    }
}

pub fn plugins_db_path(config_dir: &Path) -> PathBuf {
    config_dir.join("plugins.json")
}

pub fn marketplace_cache_dir(cache_dir: &Path, name: &str) -> PathBuf {
    cache_dir.join("marketplaces").join(name)
}

pub fn plugin_install_dir(config_dir: &Path, name: &str) -> PathBuf {
    config_dir.join("plugins").join("installed").join(name)
}

pub async fn load_plugins_db(config_dir: &Path) -> PluginsDb {
    let path = plugins_db_path(config_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => PluginsDb::default(),
    }
}

pub async fn save_plugins_db(config_dir: &Path, db: &PluginsDb) -> Result<(), String> {
    let path = plugins_db_path(config_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| format!("create dir {:?}: {}", parent, e))?;
    }
    let content = serde_json::to_string_pretty(db)
        .map_err(|e| format!("serialize plugins db: {}", e))?;
    let tmp_path = path.with_extension("tmp");
    tokio::fs::write(&tmp_path, &content).await
        .map_err(|e| format!("write {:?}: {}", tmp_path, e))?;
    tokio::fs::rename(&tmp_path, &path).await
        .map_err(|e| format!("rename {:?} -> {:?}: {}", tmp_path, path, e))?;
    Ok(())
}

pub fn parse_marketplace_json(content: &str) -> Result<MarketplaceJson, String> {
    serde_json::from_str::<MarketplaceJson>(content)
        .map_err(|e| format!("parse marketplace.json: {}", e))
}

pub async fn load_marketplace_json(dir: &Path) -> Result<MarketplaceJson, String> {
    let claude_plugin_path = dir.join(".claude-plugin").join("marketplace.json");
    let root_path = dir.join("marketplace.json");
    let path = if claude_plugin_path.exists() {
        claude_plugin_path
    } else {
        root_path
    };
    let content = tokio::fs::read_to_string(&path).await
        .map_err(|e| format!("read {:?}: {}", path, e))?;
    parse_marketplace_json(&content)
}

async fn copy_dir_recursive(src: &Path, dst: &Path, size_acc: &mut u64) -> Result<(), String> {
    tokio::fs::create_dir_all(dst).await
        .map_err(|e| format!("mkdir {:?}: {}", dst, e))?;
    let mut entries = tokio::fs::read_dir(src).await
        .map_err(|e| format!("readdir {:?}: {}", src, e))?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            tracing::warn!("skipping symlink {:?} during plugin copy", src_path);
            continue;
        }
        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path, size_acc)).await?;
        } else if file_type.is_file() {
            let file_len = tokio::fs::metadata(&src_path).await
                .map(|m| m.len())
                .unwrap_or(0);
            *size_acc += file_len;
            if *size_acc > PLUGIN_SIZE_LIMIT {
                return Err("Plugin directory exceeds 50MB size limit".to_string());
            }
            tokio::fs::copy(&src_path, &dst_path).await
                .map_err(|e| format!("copy {:?}: {}", src_path, e))?;
        }
    }
    Ok(())
}

fn is_local_source(source: &str) -> bool {
    source.starts_with('/') || source.starts_with("./") || source.starts_with("../")
}

async fn fetch_marketplace_to_cache(
    source: &str,
    cache_dir: &Path,
    tmp_name: &str,
) -> Result<PathBuf, String> {
    let target_dir = marketplace_cache_dir(cache_dir, tmp_name);
    if is_local_source(source) {
        return Ok(PathBuf::from(source));
    }
    let url = format!("https://github.com/{}.git", source);
    tokio::fs::create_dir_all(cache_dir.join("marketplaces")).await
        .map_err(|e| format!("mkdir marketplaces: {}", e))?;
    if target_dir.exists() {
        let repo = git2::Repository::open(&target_dir)
            .map_err(|e| format!("open repo {:?}: {}", target_dir, e))?;
        let mut remote = repo.find_remote("origin")
            .map_err(|e| format!("find remote: {}", e))?;
        remote.fetch(&["HEAD:refs/heads/main", "HEAD:refs/heads/master"], None, None)
            .or_else(|_| remote.fetch(&["HEAD"], None, None))
            .map_err(|e| format!("fetch: {}", e))?;
    } else {
        git2::Repository::clone(&url, &target_dir)
            .map_err(|e| format!("clone {}: {}", url, e))?;
    }
    Ok(target_dir)
}

pub async fn add_marketplace(
    gcx: Arc<ARwLock<GlobalContext>>,
    source: &str,
) -> Result<MarketplaceJson, String> {
    let (config_dir, cache_dir) = {
        let g = gcx.read().await;
        (g.config_dir.clone(), g.cache_dir.clone())
    };
    if !is_local_source(source) {
        validate_github_source(source)?;
    }
    let tmp_name = format!("tmp_marketplace_{}", uuid::Uuid::new_v4().simple());
    let marketplace_dir = fetch_marketplace_to_cache(source, &cache_dir, &tmp_name).await
        .map_err(|e| {
            let tmp_dir = marketplace_cache_dir(&cache_dir, &tmp_name);
            if tmp_dir.exists() {
                let _ = std::fs::remove_dir_all(&tmp_dir);
            }
            e
        })?;
    let mj = load_marketplace_json(&marketplace_dir).await
        .map_err(|e| {
            let tmp_dir = marketplace_cache_dir(&cache_dir, &tmp_name);
            if tmp_dir.exists() {
                let _ = std::fs::remove_dir_all(&tmp_dir);
            }
            format!("marketplace.json: {}", e)
        })?;
    let name = if mj.name.is_empty() {
        source.trim_matches('/').replace('/', "_")
    } else {
        mj.name.clone()
    };
    validate_plugin_name(&name)
        .map_err(|e| format!("invalid marketplace name '{}': {}", name, e))?;
    for plugin in &mj.plugins {
        validate_plugin_name(&plugin.name)
            .map_err(|e| format!("invalid plugin name '{}': {}", plugin.name, e))?;
    }
    if !is_local_source(source) {
        let final_dir = marketplace_cache_dir(&cache_dir, &name);
        if final_dir != marketplace_dir {
            let tmp_dir = marketplace_cache_dir(&cache_dir, &tmp_name);
            if tmp_dir.exists() {
                tokio::fs::rename(&tmp_dir, &final_dir).await
                    .map_err(|e| format!("rename marketplace dir: {}", e))?;
            }
        }
    }
    let mut db = load_plugins_db(&config_dir).await;
    db.marketplaces.retain(|m| m.name != name);
    db.marketplaces.push(MarketplaceEntry {
        name: name.clone(),
        source: source.to_string(),
        added_at: Utc::now().to_rfc3339(),
    });
    save_plugins_db(&config_dir, &db).await?;
    Ok(MarketplaceJson { name, ..mj })
}

pub async fn remove_marketplace(
    gcx: Arc<ARwLock<GlobalContext>>,
    name: &str,
) -> Result<(), String> {
    validate_plugin_name(name)?;
    let config_dir = gcx.read().await.config_dir.clone();
    let mut db = load_plugins_db(&config_dir).await;
    db.marketplaces.retain(|m| m.name != name);
    save_plugins_db(&config_dir, &db).await
}

pub async fn list_marketplace_plugins(
    gcx: Arc<ARwLock<GlobalContext>>,
    name: &str,
) -> Result<Vec<MarketplacePluginEntry>, String> {
    validate_plugin_name(name)?;
    let (config_dir, cache_dir) = {
        let g = gcx.read().await;
        (g.config_dir.clone(), g.cache_dir.clone())
    };
    let db = load_plugins_db(&config_dir).await;
    let entry = db.marketplaces.iter().find(|m| m.name == name)
        .ok_or_else(|| format!("marketplace '{}' not found", name))?;
    let marketplace_dir = if is_local_source(&entry.source) {
        PathBuf::from(&entry.source)
    } else {
        marketplace_cache_dir(&cache_dir, name)
    };
    let mj = load_marketplace_json(&marketplace_dir).await?;
    Ok(mj.plugins)
}

pub async fn install_plugin(
    gcx: Arc<ARwLock<GlobalContext>>,
    plugin_name: &str,
    marketplace_name: &str,
) -> Result<InstalledPluginEntry, String> {
    validate_plugin_name(plugin_name)?;
    validate_plugin_name(marketplace_name)?;
    let (config_dir, cache_dir) = {
        let g = gcx.read().await;
        (g.config_dir.clone(), g.cache_dir.clone())
    };
    let db = load_plugins_db(&config_dir).await;
    let market_entry = db.marketplaces.iter().find(|m| m.name == marketplace_name)
        .ok_or_else(|| format!("marketplace '{}' not found", marketplace_name))?;
    let marketplace_dir = if is_local_source(&market_entry.source) {
        PathBuf::from(&market_entry.source)
    } else {
        marketplace_cache_dir(&cache_dir, marketplace_name)
    };
    let mj = load_marketplace_json(&marketplace_dir).await?;
    let plugin_entry = mj.plugins.iter().find(|p| p.name == plugin_name)
        .ok_or_else(|| format!("plugin '{}' not found in marketplace '{}'", plugin_name, marketplace_name))?;
    let plugin_source_dir = resolve_plugin_source_dir(&marketplace_dir, &plugin_entry.source)?;
    let install_dir = plugin_install_dir(&config_dir, plugin_name);
    if install_dir.exists() {
        return Err(format!("plugin '{}' is already installed", plugin_name));
    }
    let temp_dir = install_dir.with_extension("installing");
    if temp_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
    let mut size_acc = 0u64;
    if let Err(e) = copy_dir_recursive(&plugin_source_dir, &temp_dir, &mut size_acc).await {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return Err(e);
    }
    if let Err(e) = tokio::fs::rename(&temp_dir, &install_dir).await {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return Err(format!("rename install dir: {}", e));
    }
    let version = plugin_entry.version.clone();
    let entry = InstalledPluginEntry {
        name: plugin_name.to_string(),
        marketplace: marketplace_name.to_string(),
        version,
        install_dir: install_dir.to_string_lossy().to_string(),
        installed_at: Utc::now().to_rfc3339(),
    };
    let mut db = load_plugins_db(&config_dir).await;
    db.installed.retain(|i| i.name != plugin_name);
    db.installed.push(entry.clone());
    save_plugins_db(&config_dir, &db).await?;
    gcx.read().await.ext_cache_generation.fetch_add(1, Ordering::Relaxed);
    Ok(entry)
}

fn resolve_plugin_source_dir(
    marketplace_dir: &Path,
    source: &serde_json::Value,
) -> Result<PathBuf, String> {
    match source {
        serde_json::Value::String(s) => {
            let relative = s.trim_start_matches("./");
            if Path::new(relative).is_absolute() || relative.contains("..") {
                return Err(format!("unsafe plugin source path: {}", s));
            }
            Ok(marketplace_dir.join(relative))
        }
        serde_json::Value::Object(obj) => {
            let kind = obj.get("source").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "github" {
                let repo = obj.get("repo").and_then(|v| v.as_str())
                    .ok_or("missing repo field")?;
                Err(format!("github plugin source not yet supported: {}", repo))
            } else {
                Err(format!("unsupported plugin source type: {}", kind))
            }
        }
        _ => Err("invalid plugin source field".to_string()),
    }
}

pub async fn uninstall_plugin(
    gcx: Arc<ARwLock<GlobalContext>>,
    plugin_name: &str,
) -> Result<(), String> {
    validate_plugin_name(plugin_name)?;
    let config_dir = gcx.read().await.config_dir.clone();
    let mut db = load_plugins_db(&config_dir).await;
    let was_installed = db.installed.iter().any(|i| i.name == plugin_name);
    db.installed.retain(|i| i.name != plugin_name);
    save_plugins_db(&config_dir, &db).await?;
    let install_dir = plugin_install_dir(&config_dir, plugin_name);
    if install_dir.exists() {
        tokio::fs::remove_dir_all(&install_dir).await
            .map_err(|e| format!("remove install dir {:?}: {}", install_dir, e))?;
    }
    if was_installed {
        gcx.read().await.ext_cache_generation.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext::config_dirs::CommandSource;

    #[test]
    fn test_validate_plugin_name_accepts_valid() {
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("plugin_v2").is_ok());
        assert!(validate_plugin_name("plugin.1.0").is_ok());
        assert!(validate_plugin_name("abc").is_ok());
        assert!(validate_plugin_name("Plugin-123").is_ok());
    }

    #[test]
    fn test_validate_plugin_name_rejects_traversal() {
        assert!(validate_plugin_name("../../etc").is_err());
        assert!(validate_plugin_name("/absolute").is_err());
        assert!(validate_plugin_name("a/b").is_err());
        assert!(validate_plugin_name("..").is_err());
        assert!(validate_plugin_name(".hidden").is_err());
        assert!(validate_plugin_name("").is_err());
        assert!(validate_plugin_name("path\\traversal").is_err());
        assert!(validate_plugin_name("name with spaces").is_err());
    }

    #[tokio::test]
    async fn test_copy_dir_recursive_skips_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        tokio::fs::create_dir_all(&src).await.unwrap();

        tokio::fs::write(src.join("regular.txt"), "content").await.unwrap();

        #[cfg(unix)]
        {
            let target = tmp.path().join("target_file.txt");
            tokio::fs::write(&target, "secret").await.unwrap();
            std::os::unix::fs::symlink(&target, src.join("symlink.txt")).unwrap();

            let mut size_acc = 0u64;
            copy_dir_recursive(&src, &dst, &mut size_acc).await.unwrap();

            assert!(dst.join("regular.txt").exists());
            assert!(!dst.join("symlink.txt").exists(), "symlink should not be copied");
        }
        #[cfg(not(unix))]
        {
            let mut size_acc = 0u64;
            copy_dir_recursive(&src, &dst, &mut size_acc).await.unwrap();
            assert!(dst.join("regular.txt").exists());
        }
    }

    #[tokio::test]
    async fn test_marketplace_json_claude_path() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_plugin_dir = tmp.path().join(".claude-plugin");
        tokio::fs::create_dir_all(&claude_plugin_dir).await.unwrap();
        let mj_content = r#"{"name": "test-market", "plugins": []}"#;
        tokio::fs::write(claude_plugin_dir.join("marketplace.json"), mj_content).await.unwrap();

        let mj = load_marketplace_json(tmp.path()).await.unwrap();
        assert_eq!(mj.name, "test-market");
    }

    #[tokio::test]
    async fn test_marketplace_json_fallback_root() {
        let tmp = tempfile::tempdir().unwrap();
        let mj_content = r#"{"name": "root-market", "plugins": []}"#;
        tokio::fs::write(tmp.path().join("marketplace.json"), mj_content).await.unwrap();

        let mj = load_marketplace_json(tmp.path()).await.unwrap();
        assert_eq!(mj.name, "root-market");
    }

    #[tokio::test]
    async fn test_marketplace_json_claude_path_preferred_over_root() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_plugin_dir = tmp.path().join(".claude-plugin");
        tokio::fs::create_dir_all(&claude_plugin_dir).await.unwrap();
        tokio::fs::write(claude_plugin_dir.join("marketplace.json"), r#"{"name": "claude-market"}"#).await.unwrap();
        tokio::fs::write(tmp.path().join("marketplace.json"), r#"{"name": "root-market"}"#).await.unwrap();

        let mj = load_marketplace_json(tmp.path()).await.unwrap();
        assert_eq!(mj.name, "claude-market");
    }

    #[test]
    fn test_parse_marketplace_json_valid() {
        let json = r#"{
            "name": "test-marketplace",
            "owner": {"name": "Test Author", "email": "test@example.com"},
            "plugins": [
                {
                    "name": "plugin-a",
                    "source": "./plugins/plugin-a",
                    "description": "Plugin A",
                    "version": "1.0.0",
                    "tags": ["search"]
                },
                {
                    "name": "plugin-b",
                    "source": {"source": "github", "repo": "owner/plugin-b"},
                    "description": "Plugin B",
                    "version": "2.0.0"
                }
            ]
        }"#;
        let mj = parse_marketplace_json(json).unwrap();
        assert_eq!(mj.name, "test-marketplace");
        assert_eq!(mj.plugins.len(), 2);
        assert_eq!(mj.plugins[0].name, "plugin-a");
        assert_eq!(mj.plugins[0].description, "Plugin A");
        assert_eq!(mj.plugins[0].version, "1.0.0");
        assert_eq!(mj.plugins[0].tags, vec!["search"]);
        assert_eq!(mj.plugins[1].name, "plugin-b");
    }

    #[test]
    fn test_parse_marketplace_json_missing_fields() {
        let json = r#"{"plugins": [{"name": "x"}]}"#;
        let mj = parse_marketplace_json(json).unwrap();
        assert_eq!(mj.name, "");
        assert_eq!(mj.plugins.len(), 1);
        assert_eq!(mj.plugins[0].name, "x");
        assert_eq!(mj.plugins[0].description, "");
        assert_eq!(mj.plugins[0].version, "");
    }

    #[test]
    fn test_parse_marketplace_json_invalid() {
        let result = parse_marketplace_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_marketplace_json_empty_plugins() {
        let json = r#"{"name": "empty", "plugins": []}"#;
        let mj = parse_marketplace_json(json).unwrap();
        assert_eq!(mj.name, "empty");
        assert!(mj.plugins.is_empty());
    }

    #[test]
    fn test_command_source_installed_plugin_serde() {
        let src = CommandSource::InstalledPlugin("my-plugin".to_string());
        let json = serde_json::to_string(&src).unwrap();
        let restored: CommandSource = serde_json::from_str(&json).unwrap();
        if let CommandSource::InstalledPlugin(name) = restored {
            assert_eq!(name, "my-plugin");
        } else {
            panic!("expected InstalledPlugin");
        }
    }

    #[test]
    fn test_command_source_variants_serde() {
        use std::path::PathBuf;
        let variants = vec![
            CommandSource::GlobalClaude,
            CommandSource::GlobalRefact,
            CommandSource::ProjectClaude(PathBuf::from("/proj")),
            CommandSource::ProjectRefact(PathBuf::from("/proj")),
            CommandSource::InstalledPlugin("test-plugin".to_string()),
        ];
        for src in &variants {
            let json = serde_json::to_string(src).unwrap();
            let restored: CommandSource = serde_json::from_str(&json).unwrap();
            let orig_json = serde_json::to_string(src).unwrap();
            let rest_json = serde_json::to_string(&restored).unwrap();
            assert_eq!(orig_json, rest_json, "Roundtrip failed for {:?}", src);
        }
    }

    #[tokio::test]
    async fn test_ext_dirs_includes_installed_plugin_dirs() {
        use crate::ext::config_dirs::ExtDirs;
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().to_path_buf();
        let install_root = config_dir.join("plugins").join("installed");
        tokio::fs::create_dir_all(&install_root).await.unwrap();
        tokio::fs::create_dir_all(install_root.join("plugin-x")).await.unwrap();
        tokio::fs::create_dir_all(install_root.join("plugin-y")).await.unwrap();

        let mut installed_dirs = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&install_root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    installed_dirs.push(path);
                }
            }
        }
        installed_dirs.sort();

        let ext_dirs = ExtDirs {
            global_dirs: vec![config_dir.clone()],
            installed_dirs: installed_dirs.clone(),
            project_dirs: vec![],
        };

        assert_eq!(ext_dirs.installed_dirs.len(), 2);
        let all = ext_dirs.all_dirs_in_order();
        assert!(all.contains(&&install_root.join("plugin-x")));
        assert!(all.contains(&&install_root.join("plugin-y")));

        let global_dirs = vec![config_dir.clone()];
        let src = crate::ext::config_dirs::source_for_dir(
            &install_root.join("plugin-x"),
            &global_dirs,
            &installed_dirs,
        );
        assert!(matches!(src, crate::ext::config_dirs::CommandSource::InstalledPlugin(n) if n == "plugin-x"));
    }

    #[tokio::test]
    async fn test_install_writes_to_correct_dir_and_updates_db() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        tokio::fs::create_dir_all(&config_dir).await.unwrap();

        let marketplace_dir = tmp.path().join("marketplace");
        tokio::fs::create_dir_all(&marketplace_dir).await.unwrap();

        let plugin_src = marketplace_dir.join("plugins").join("my-plugin");
        tokio::fs::create_dir_all(&plugin_src).await.unwrap();
        tokio::fs::write(plugin_src.join("SKILL.md"), "---\nname: test\ndescription: test skill\n---\nBody").await.unwrap();

        let marketplace_json = serde_json::json!({
            "name": "test-market",
            "plugins": [{
                "name": "my-plugin",
                "source": "./plugins/my-plugin",
                "description": "Test plugin",
                "version": "1.0.0"
            }]
        });
        tokio::fs::write(
            marketplace_dir.join("marketplace.json"),
            serde_json::to_string(&marketplace_json).unwrap(),
        ).await.unwrap();

        let db_before = PluginsDb {
            marketplaces: vec![MarketplaceEntry {
                name: "test-market".to_string(),
                source: marketplace_dir.to_string_lossy().to_string(),
                added_at: "2024-01-01T00:00:00+00:00".to_string(),
            }],
            installed: vec![],
        };
        save_plugins_db(&config_dir, &db_before).await.unwrap();

        let install_dir = plugin_install_dir(&config_dir, "my-plugin");
        assert!(!install_dir.exists());

        let mut size_acc = 0u64;
        copy_dir_recursive(&plugin_src, &install_dir, &mut size_acc).await.unwrap();

        let mut db = load_plugins_db(&config_dir).await;
        db.installed.push(InstalledPluginEntry {
            name: "my-plugin".to_string(),
            marketplace: "test-market".to_string(),
            version: "1.0.0".to_string(),
            install_dir: install_dir.to_string_lossy().to_string(),
            installed_at: Utc::now().to_rfc3339(),
        });
        save_plugins_db(&config_dir, &db).await.unwrap();

        assert!(install_dir.exists());
        assert!(install_dir.join("SKILL.md").exists());

        let db_after = load_plugins_db(&config_dir).await;
        assert_eq!(db_after.installed.len(), 1);
        assert_eq!(db_after.installed[0].name, "my-plugin");
        assert_eq!(db_after.installed[0].marketplace, "test-market");
    }

    #[tokio::test]
    async fn test_plugins_db_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().to_path_buf();
        let db = PluginsDb {
            marketplaces: vec![MarketplaceEntry {
                name: "market".to_string(),
                source: "/some/path".to_string(),
                added_at: "2024-01-01".to_string(),
            }],
            installed: vec![InstalledPluginEntry {
                name: "plugin".to_string(),
                marketplace: "market".to_string(),
                version: "1.0.0".to_string(),
                install_dir: "/installed/plugin".to_string(),
                installed_at: "2024-01-02".to_string(),
            }],
        };
        save_plugins_db(&config_dir, &db).await.unwrap();
        let loaded = load_plugins_db(&config_dir).await;
        assert_eq!(loaded.marketplaces.len(), 1);
        assert_eq!(loaded.marketplaces[0].name, "market");
        assert_eq!(loaded.installed.len(), 1);
        assert_eq!(loaded.installed[0].name, "plugin");
    }

    #[tokio::test]
    async fn test_load_plugins_db_missing_file() {
        let db = load_plugins_db(Path::new("/nonexistent/path")).await;
        assert!(db.marketplaces.is_empty());
        assert!(db.installed.is_empty());
    }
}
