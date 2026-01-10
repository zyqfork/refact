use std::path::PathBuf;
use std::sync::Arc;
use axum::Extension;
use axum::http::{Response, StatusCode};
use hyper::Body;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as ARwLock;
use tokio::fs;
use chrono::Local;

use crate::custom_error::ScratchError;
use crate::global_context::GlobalContext;
use crate::knowledge_graph::KnowledgeFrontmatter;
use crate::files_in_workspace::get_file_text_from_memory_or_disk;

#[derive(Deserialize)]
pub struct UpdateMemoryPost {
    pub file_path: String,
    #[serde(default)]
    pub title: Option<String>,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
}

#[derive(Deserialize)]
pub struct DeleteMemoryPost {
    pub file_path: String,
    #[serde(default)]
    pub archive: bool,
}

#[derive(Serialize)]
pub struct MemoryOperationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_v1_knowledge_update_memory(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    let post = serde_json::from_slice::<UpdateMemoryPost>(&body_bytes).map_err(|e| {
        ScratchError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("JSON problem: {}", e),
        )
    })?;

    let file_path = PathBuf::from(&post.file_path);
    
    if !file_path.exists() {
        return Err(ScratchError::new(
            StatusCode::NOT_FOUND,
            format!("Memory file not found: {}", post.file_path),
        ));
    }

    let existing_text = get_file_text_from_memory_or_disk(gcx.clone(), &file_path)
        .await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let (mut frontmatter, _) = KnowledgeFrontmatter::parse(&existing_text);

    if let Some(title) = post.title {
        frontmatter.title = Some(title);
    }
    if !post.tags.is_empty() {
        frontmatter.tags = post.tags;
    }
    if let Some(kind) = post.kind {
        frontmatter.kind = Some(kind);
    }
    if !post.filenames.is_empty() {
        frontmatter.filenames = post.filenames;
    }
    frontmatter.updated = Some(Local::now().format("%Y-%m-%d").to_string());

    let new_content = format!("{}\n\n{}", frontmatter.to_yaml(), post.content.trim());

    let tmp_path = file_path.with_extension("md.tmp");
    fs::write(&tmp_path, &new_content)
        .await
        .map_err(|e| {
            ScratchError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write temporary file: {}", e),
            )
        })?;

    fs::rename(&tmp_path, &file_path)
        .await
        .map_err(|e| {
            ScratchError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update memory file: {}", e),
            )
        })?;

    if let Some(vecdb) = gcx.read().await.vec_db.lock().await.as_ref() {
        vecdb
            .vectorizer_enqueue_files(&vec![file_path.to_string_lossy().to_string()], true)
            .await;
    }

    tracing::info!("Updated memory: {}", file_path.display());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&MemoryOperationResponse {
                success: true,
                error: None,
            })
            .unwrap(),
        ))
        .unwrap())
}

pub async fn handle_v1_knowledge_delete_memory(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    let post = serde_json::from_slice::<DeleteMemoryPost>(&body_bytes).map_err(|e| {
        ScratchError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("JSON problem: {}", e),
        )
    })?;

    let file_path = PathBuf::from(&post.file_path);
    
    if !file_path.exists() {
        return Err(ScratchError::new(
            StatusCode::NOT_FOUND,
            format!("Memory file not found: {}", post.file_path),
        ));
    }

    if post.archive {
        crate::memories::archive_document(gcx.clone(), &file_path)
            .await
            .map_err(|e| {
                ScratchError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to archive memory: {}", e),
                )
            })?;
        tracing::info!("Archived memory: {}", file_path.display());
    } else {
        fs::remove_file(&file_path)
            .await
            .map_err(|e| {
                ScratchError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete memory file: {}", e),
                )
            })?;
        tracing::info!("Deleted memory: {}", file_path.display());
    }

    if let Some(vecdb) = gcx.read().await.vec_db.lock().await.as_ref() {
        vecdb
            .vectorizer_enqueue_files(&vec![file_path.to_string_lossy().to_string()], true)
            .await;
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&MemoryOperationResponse {
                success: true,
                error: None,
            })
            .unwrap(),
        ))
        .unwrap())
}
