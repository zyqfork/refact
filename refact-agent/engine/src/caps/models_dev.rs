#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as ARwLock;
use tracing::warn;

use crate::global_context::GlobalContext;
use crate::providers::traits::ModelPricing;

pub const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";
const MODELS_DEV_CACHE_DIR: &str = "models_dev";
const MODELS_DEV_CACHE_FILE: &str = "api.json";
const FETCH_TIMEOUT_SECS: u64 = 10;
const MODELS_DEV_SNAPSHOT: &str = include_str!("models_dev_snapshot.json");

pub type ModelsDevCatalog = HashMap<String, ModelsDevProvider>;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelsDevProvider {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, ModelsDevModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelsDevModel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub cost: Option<ModelsDevCost>,
    #[serde(default)]
    pub limit: Option<ModelsDevLimit>,
    #[serde(default)]
    pub modalities: Option<ModelsDevModalities>,
    #[serde(default)]
    pub provider: Option<ModelsDevModelProvider>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub experimental: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelsDevCost {
    #[serde(default)]
    pub input: Option<f64>,
    #[serde(default)]
    pub output: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
    #[serde(default)]
    pub cache_write: Option<f64>,
    #[serde(default)]
    pub context_over_200k: Option<ModelsDevCostTier>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelsDevCostTier {
    #[serde(default)]
    pub input: Option<f64>,
    #[serde(default)]
    pub output: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
    #[serde(default)]
    pub cache_write: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelsDevLimit {
    #[serde(default)]
    pub context: Option<usize>,
    #[serde(default)]
    pub input: Option<usize>,
    #[serde(default)]
    pub output: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelsDevModalities {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelsDevModelProvider {
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub npm: Option<String>,
}

pub fn parse_catalog_json(json: &str) -> Result<ModelsDevCatalog, String> {
    let catalog: ModelsDevCatalog = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse models.dev catalog: {e}"))?;
    normalize_and_validate_catalog(catalog)
}

pub fn load_models_dev_snapshot_catalog() -> Result<ModelsDevCatalog, String> {
    parse_catalog_json(MODELS_DEV_SNAPSHOT)
        .map_err(|e| format!("Failed to parse bundled models.dev snapshot: {e}"))
}

pub fn models_dev_cache_path(cache_dir: &Path) -> PathBuf {
    cache_dir
        .join(MODELS_DEV_CACHE_DIR)
        .join(MODELS_DEV_CACHE_FILE)
}

pub fn get_provider<'a>(
    catalog: &'a ModelsDevCatalog,
    provider_id: &str,
) -> Option<&'a ModelsDevProvider> {
    catalog
        .get(provider_id)
        .or_else(|| catalog.values().find(|provider| provider.id == provider_id))
}

pub fn get_model<'a>(
    catalog: &'a ModelsDevCatalog,
    provider_id: &str,
    model_id: &str,
) -> Option<&'a ModelsDevModel> {
    let provider = get_provider(catalog, provider_id)?;
    provider
        .models
        .get(model_id)
        .or_else(|| provider.models.values().find(|model| model.id == model_id))
}

pub fn model_cost_to_pricing(model: &ModelsDevModel) -> Option<ModelPricing> {
    cost_to_pricing(model.cost.as_ref()?)
}

pub fn cost_to_pricing(cost: &ModelsDevCost) -> Option<ModelPricing> {
    let pricing = ModelPricing {
        prompt: cost.input?,
        generated: cost.output?,
        cache_read: cost.cache_read,
        cache_creation: cost.cache_write,
    };
    pricing.is_valid().then_some(pricing)
}

pub async fn load_models_dev_catalog(
    gcx: Arc<ARwLock<GlobalContext>>,
    force_refresh: bool,
) -> Result<ModelsDevCatalog, String> {
    let (cache_dir, http_client) = {
        let gcx_locked = gcx.read().await;
        (gcx_locked.cache_dir.clone(), gcx_locked.http_client.clone())
    };

    if force_refresh {
        match fetch_models_dev_catalog(&http_client).await {
            Ok((catalog, body)) => {
                if let Err(e) = write_models_dev_cache(&cache_dir, &body).await {
                    warn!("Failed to write models.dev runtime cache: {e}");
                }
                Ok(catalog)
            }
            Err(e) => {
                warn!("Failed to refresh models.dev catalog: {e}; using cache or snapshot");
                load_models_dev_catalog_from_cache_or_snapshot(&cache_dir).await
            }
        }
    } else {
        load_models_dev_catalog_from_cache_or_snapshot(&cache_dir).await
    }
}

pub async fn load_models_dev_catalog_from_cache_or_snapshot(
    cache_dir: &Path,
) -> Result<ModelsDevCatalog, String> {
    let cache_path = models_dev_cache_path(cache_dir);
    match tokio::fs::read_to_string(&cache_path).await {
        Ok(contents) => match parse_catalog_json(&contents) {
            Ok(catalog) => Ok(catalog),
            Err(e) => {
                warn!(
                    "models.dev runtime cache '{}' is corrupt: {e}; using bundled snapshot",
                    cache_path.display()
                );
                load_models_dev_snapshot_catalog()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => load_models_dev_snapshot_catalog(),
        Err(e) => {
            warn!(
                "Failed to read models.dev runtime cache '{}': {e}; using bundled snapshot",
                cache_path.display()
            );
            load_models_dev_snapshot_catalog()
        }
    }
}

pub async fn fetch_models_dev_catalog(
    http_client: &reqwest::Client,
) -> Result<(ModelsDevCatalog, String), String> {
    tokio::time::timeout(Duration::from_secs(FETCH_TIMEOUT_SECS), async {
        let response = http_client
            .get(MODELS_DEV_API_URL)
            .header(USER_AGENT, "refact-lsp models.dev catalog")
            .send()
            .await
            .map_err(|e| format!("Failed to request models.dev catalog: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("models.dev catalog returned HTTP {status}"));
        }
        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read models.dev catalog response: {e}"))?;
        let catalog = parse_catalog_json(&body)?;
        Ok((catalog, body))
    })
    .await
    .map_err(|_| "Timed out fetching models.dev catalog".to_string())?
}

pub async fn write_models_dev_cache(cache_dir: &Path, contents: &str) -> Result<(), String> {
    let cache_path = models_dev_cache_path(cache_dir);
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create models.dev cache directory: {e}"))?;
    }
    let tmp_path = cache_path.with_extension("json.tmp");
    tokio::fs::write(&tmp_path, contents)
        .await
        .map_err(|e| format!("Failed to write models.dev cache temp file: {e}"))?;
    tokio::fs::rename(&tmp_path, &cache_path)
        .await
        .map_err(|e| format!("Failed to replace models.dev cache file: {e}"))?;
    Ok(())
}

fn normalize_and_validate_catalog(
    mut catalog: ModelsDevCatalog,
) -> Result<ModelsDevCatalog, String> {
    if catalog.is_empty() {
        return Err("models.dev catalog is empty".to_string());
    }

    let mut model_count = 0usize;
    for (provider_key, provider) in catalog.iter_mut() {
        if provider.id.is_empty() {
            provider.id = provider_key.clone();
        }
        for (model_key, model) in provider.models.iter_mut() {
            if model.id.is_empty() {
                model.id = model_key.clone();
            }
            model_count += 1;
        }
    }

    if model_count == 0 {
        return Err("models.dev catalog contains no models".to_string());
    }

    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_catalog_json() -> &'static str {
        r#"
        {
            "openai": {
                "id": "openai",
                "name": "OpenAI",
                "env": ["OPENAI_API_KEY"],
                "api": "https://api.openai.com/v1",
                "npm": "@ai-sdk/openai",
                "models": {
                    "gpt-4o": {
                        "id": "gpt-4o",
                        "name": "GPT-4o",
                        "family": "gpt",
                        "reasoning": false,
                        "temperature": true,
                        "tool_call": true,
                        "cost": {
                            "input": 2.5,
                            "output": 10.0,
                            "cache_read": 1.25,
                            "cache_write": 3.75
                        },
                        "limit": {
                            "context": 128000,
                            "output": 16384
                        },
                        "modalities": {
                            "input": ["text", "image"],
                            "output": ["text"]
                        },
                        "provider": {
                            "api": "gpt-4o"
                        },
                        "status": "beta",
                        "experimental": {
                            "modes": {}
                        },
                        "unknown_future_field": true
                    }
                },
                "unknown_provider_field": "ignored"
            }
        }
        "#
    }

    #[test]
    fn minimal_catalog_parses_successfully() {
        let catalog = parse_catalog_json(minimal_catalog_json()).unwrap();
        let provider = get_provider(&catalog, "openai").unwrap();
        assert_eq!(provider.name, "OpenAI");
        assert_eq!(provider.env, vec!["OPENAI_API_KEY"]);
        let model = get_model(&catalog, "openai", "gpt-4o").unwrap();
        assert_eq!(model.name, "GPT-4o");
        assert!(model.temperature);
        assert!(model.tool_call);
        assert_eq!(model.family.as_deref(), Some("gpt"));
        assert_eq!(model.status.as_deref(), Some("beta"));
        assert_eq!(
            model.limit.as_ref().and_then(|limit| limit.context),
            Some(128000)
        );
    }

    #[test]
    fn provider_and_model_lookup_uses_ids() {
        let catalog = parse_catalog_json(
            r#"
            {
                "provider-key": {
                    "id": "provider-id",
                    "name": "Provider",
                    "models": {
                        "model-key": {
                            "id": "model-id",
                            "name": "Model"
                        }
                    }
                }
            }
            "#,
        )
        .unwrap();

        assert!(get_provider(&catalog, "provider-key").is_some());
        assert!(get_provider(&catalog, "provider-id").is_some());
        assert!(get_model(&catalog, "provider-id", "model-key").is_some());
        assert!(get_model(&catalog, "provider-id", "model-id").is_some());
        assert!(get_model(&catalog, "provider-id", "missing").is_none());
    }

    #[tokio::test]
    async fn corrupt_cache_falls_back_to_snapshot() {
        let tempdir = tempfile::tempdir().unwrap();
        let cache_path = models_dev_cache_path(tempdir.path());
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, "not json").unwrap();

        let catalog = load_models_dev_catalog_from_cache_or_snapshot(tempdir.path())
            .await
            .unwrap();

        assert!(!catalog.is_empty());
    }

    #[test]
    fn cost_conversion_maps_to_model_pricing() {
        let catalog = parse_catalog_json(minimal_catalog_json()).unwrap();
        let model = get_model(&catalog, "openai", "gpt-4o").unwrap();
        let pricing = model_cost_to_pricing(model).unwrap();

        assert_eq!(pricing.prompt, 2.5);
        assert_eq!(pricing.generated, 10.0);
        assert_eq!(pricing.cache_read, Some(1.25));
        assert_eq!(pricing.cache_creation, Some(3.75));
    }

    #[test]
    fn incomplete_cost_does_not_convert_to_pricing() {
        let cost = ModelsDevCost {
            input: Some(1.0),
            output: None,
            ..Default::default()
        };

        assert!(cost_to_pricing(&cost).is_none());
    }

    #[test]
    fn generated_snapshot_parses() {
        let catalog = load_models_dev_snapshot_catalog().unwrap();

        assert!(!catalog.is_empty());
        assert!(get_provider(&catalog, "openai").is_some());
    }
}
