use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;

pub async fn bearer_auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    // /health, /human/{token}, /, and login/session are accessible for web visitors & healthchecks
    if path == "/api/v1/health"
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/session"
        || path.starts_with("/human/")
        || path == "/"
    {
        return next.run(req).await;
    }

    // Check user session token from SQLite KV
    let token_candidate = req
        .headers()
        .get("X-Sinopec-Auth-Token")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            req.headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
        })
        .map(|s| s.trim());

    if let Some(tok) = token_candidate {
        if let Ok(Some(_)) = state.store.get_kv(&format!("session:{}", tok)).await {
            return next.run(req).await;
        }
    }

    let Some(expected) = &state.config.api_token else {
        return next.run(req).await;
    };

    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim());

    if provided == Some(expected.as_str()) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "ok": false,
                "error": {
                    "code": "LOGIN_REQUIRED",
                    "message": "Missing or invalid Authorization: Bearer <SINOPEC_API_TOKEN> header"
                }
            })),
        )
            .into_response()
    }
}
