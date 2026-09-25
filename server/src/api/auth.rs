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
    // /health and /human/{token} are always accessible for container healthchecks & user browser clicks
    if path == "/api/v1/health" || path.starts_with("/human/") {
        return next.run(req).await;
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
