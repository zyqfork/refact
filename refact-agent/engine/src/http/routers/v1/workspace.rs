use std::sync::Arc;
use axum::Extension;
use axum::http::{Response, StatusCode};
use hyper::Body;
use tokio::sync::RwLock as ARwLock;

use crate::custom_error::ScratchError;
use crate::global_context::GlobalContext;

pub async fn handle_v1_get_app_searchable_id(
    Extension(gcx): Extension<Arc<ARwLock<GlobalContext>>>,
    _body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "app_searchable_id": gcx.read().await.app_searchable_id }),
            )
            .unwrap(),
        ))
        .unwrap())
}
