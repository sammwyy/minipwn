//! Router construction for the worker HTTP server with bearer token auth.

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde_json::json;

use super::handlers::*;
use super::state::AppState;
use crate::config::WorkerConfig;

/// Build the Axum router with auth middleware.
pub fn build_router(config: WorkerConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/info", get(handle_info))
        .route("/exec", post(handle_exec))
        .route("/shell/open", post(handle_shell_open))
        .route("/shell/write", post(handle_shell_write))
        .route("/shell/read", post(handle_shell_read))
        .route("/shell/close", post(handle_shell_close))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

/// Bearer token authentication middleware.
async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let expected = format!("Bearer {}", state.config.server.secret);

    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if auth != expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized" })),
        ));
    }

    Ok(next.run(req).await)
}
