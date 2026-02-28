use std::sync::Arc;
use axum::Extension;
use axum::extract::Path;
use axum::response::Json;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock as ARwLock;

use crate::custom_error::ScratchError;
use crate::ext::plugins::{
    add_marketplace, install_plugin, list_marketplace_plugins, load_plugins_db,
    remove_marketplace, uninstall_plugin,
};
use crate::global_context::GlobalContext;

#[derive(Deserialize)]
pub struct AddMarketplaceRequest {
    pub source: String,
}

#[derive(Deserialize)]
pub struct InstallPluginRequest {
    pub plugin: String,
    pub marketplace: String,
}

#[derive(Serialize)]
pub struct MarketplaceSummary {
    pub name: String,
    pub source: String,
    pub added_at: String,
    pub plugin_count: usize,
}

pub async fn handle_list_marketplaces(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let config_dir = gcx.read().await.config_dir.clone();
    let db = load_plugins_db(&config_dir).await;
    let summaries: Vec<Value> = db.marketplaces.iter().map(|m| {
        json!({
            "name": m.name,
            "source": m.source,
            "added_at": m.added_at,
        })
    }).collect();
    Ok(Json(json!({ "marketplaces": summaries })))
}

pub async fn handle_add_marketplace(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Json<Value>, ScratchError> {
    let req = serde_json::from_slice::<AddMarketplaceRequest>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON: {}", e)))?;
    let mj = add_marketplace(gcx, &req.source).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({
        "name": mj.name,
        "plugin_count": mj.plugins.len(),
    })))
}

pub async fn handle_delete_marketplace(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    remove_marketplace(gcx, &name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "deleted": true })))
}

pub async fn handle_list_marketplace_plugins(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let plugins = list_marketplace_plugins(gcx, &name).await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    Ok(Json(json!({ "plugins": plugins })))
}

pub async fn handle_install_plugin(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Json<Value>, ScratchError> {
    let req = serde_json::from_slice::<InstallPluginRequest>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON: {}", e)))?;
    let entry = install_plugin(gcx, &req.plugin, &req.marketplace).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({
        "name": entry.name,
        "marketplace": entry.marketplace,
        "version": entry.version,
        "install_dir": entry.install_dir,
        "installed_at": entry.installed_at,
    })))
}

pub async fn handle_list_installed(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let config_dir = gcx.read().await.config_dir.clone();
    let db = load_plugins_db(&config_dir).await;
    Ok(Json(json!({ "installed": db.installed })))
}

pub async fn handle_uninstall_plugin(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    uninstall_plugin(gcx, &name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "deleted": true })))
}
