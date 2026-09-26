use crate::browser::driver::BrowserDriver;
use axum::{
    extract::{Path, Query, State},
    middleware,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::api::{auth::bearer_auth_middleware, human_ui::render_human_action_page};
use crate::auth::credentials::StoredCredentials;
use crate::error::{ApiEnvelope, AppError, AppResult, ErrorCode};
use crate::mcp::{mcp_get_handler, mcp_post_handler};
use crate::sinopec::models::{CreateInvoiceRequest, InvoicePreviewRequest};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TransactionsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub card_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteHumanActionBody {
    pub sms_code: Option<String>,
    pub credentials: Option<StoredCredentials>,
}

pub fn build_router(state: AppState) -> Router {
    let api_v1 = Router::new()
        .route("/health", get(health_handler))
        .route("/system", get(system_handler))
        .route("/auth/status", get(auth_status_handler))
        .route("/auth/ensure", post(auth_ensure_handler))
        .route("/auth/check", post(auth_check_handler))
        .route("/auth/send-sms", post(auth_send_sms_handler))
        .route("/account", get(account_handler))
        .route("/cards", get(cards_handler))
        .route("/invoice-capabilities", get(invoice_capabilities_handler))
        .route("/invoice-quota", get(invoice_quota_handler))
        .route("/transactions", get(transactions_handler))
        .route("/recharges", get(recharges_handler))
        .route("/allocations", get(allocations_handler))
        .route("/browser/prewarm", post(browser_prewarm_handler))
        .route("/invoices/create-live", post(invoice_create_live_handler))
        .route("/invoices/preview", post(invoice_preview_handler))
        .route(
            "/invoices",
            post(invoice_create_handler).get(invoices_list_handler),
        )
        .route("/invoices/{id}", get(invoice_detail_handler))
        .route("/invoices/{id}/download", post(invoice_download_handler))
        .route("/operations/{id}", get(operation_detail_handler))
        .route("/human-actions", get(human_actions_list_handler))
        .route("/human-actions/{id}", get(human_action_detail_handler))
        .route(
            "/human-actions/{id}/complete",
            post(human_action_complete_handler),
        )
        .route(
            "/human-actions/{id}/cancel",
            post(human_action_cancel_handler),
        )
        .route("/settings", get(settings_get_handler))
        .route(
            "/settings/credentials",
            put(settings_put_credentials_handler).delete(settings_delete_credentials_handler),
        )
        .route("/research/status", get(research_status_handler))
        .route("/research/endpoints", get(research_endpoints_handler));

    Router::new()
        .nest("/api/v1", api_v1)
        .route("/human/{token}", get(render_human_action_page))
        .route("/mcp", get(mcp_get_handler).post(mcp_post_handler))
        .route("/", get(root_redirect_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bearer_auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let auth = state.service.auth_status().await?;
    Ok(ApiEnvelope::success(json!({
        "service": "sinopec-server",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "healthy",
        "auth_state": auth.state,
        "account_mode": auth.account_mode,
        "research_mode": state.config.research_mode,
        "demo_mode": state.config.demo_mode,
        "allow_auto_submit": state.config.allow_auto_submit
    })))
}

async fn system_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let steel = state.browser.check_health().await;
    let scrapling = state.scrapling.status().await;
    let auth = state.service.auth_status().await?;
    let latest_op = state.store.latest_operation().await?;

    Ok(ApiEnvelope::success(json!({
        "backend": {
            "name": "sinopec-server",
            "version": env!("CARGO_PKG_VERSION"),
            "bind": state.config.bind_addr,
            "data_dir": state.config.data_dir,
            "sqlite_db": state.config.db_path(),
            "research_mode": state.config.research_mode,
            "demo_mode": state.config.demo_mode,
            "allow_auto_submit": state.config.allow_auto_submit,
            "auth_mode": state.config.auth_mode
        },
        "auth": auth,
        "steel": steel,
        "scrapling": scrapling,
        "last_operation": latest_op
    })))
}

async fn auth_status_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let status = state.service.auth_status().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(status).unwrap_or_default(),
    ))
}

async fn auth_ensure_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let status = state.service.ensure_auth(None).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(status).unwrap_or_default(),
    ))
}

async fn auth_check_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let status = state.service.check_auth().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(status).unwrap_or_default(),
    ))
}

async fn account_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let acc = state.service.account_info().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(acc).unwrap_or_default(),
    ))
}

async fn cards_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let cards = state.service.list_cards().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(cards).unwrap_or_default(),
    ))
}

async fn invoice_capabilities_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let caps = state.service.detect_invoice_capabilities().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(caps).unwrap_or_default(),
    ))
}

async fn invoice_quota_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let quota = state.service.invoice_quota().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(quota).unwrap_or_default(),
    ))
}

async fn transactions_handler(
    State(state): State<AppState>,
    Query(q): Query<TransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.unwrap_or_else(|| "2026-09-01".to_string());
    let end = q.end_date.unwrap_or_else(|| "2026-09-30".to_string());
    let list = state
        .service
        .list_transactions(&start, &end, q.card_id.as_deref())
        .await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(list).unwrap_or_default(),
    ))
}

async fn invoice_preview_handler(
    State(state): State<AppState>,
    Json(req): Json<InvoicePreviewRequest>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let preview = state.service.preview_invoice(req).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(preview).unwrap_or_default(),
    ))
}

async fn invoice_create_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateInvoiceRequest>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let created = state.service.create_invoice(req).await?;
    Ok(ApiEnvelope::success(created))
}

async fn invoices_list_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let list = state.service.list_invoices().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(list).unwrap_or_default(),
    ))
}

async fn invoice_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let item = state.service.invoice_status(&id).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(item).unwrap_or_default(),
    ))
}

async fn invoice_download_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let dl = state.service.download_invoice(&id).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(dl).unwrap_or_default(),
    ))
}

async fn operation_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let op = state.store.get_operation(&id).await?.ok_or_else(|| {
        AppError::new(
            ErrorCode::InvalidRequest,
            format!("Operation not found: {id}"),
        )
    })?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(op).unwrap_or_default(),
    ))
}

async fn human_actions_list_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let list = state.human.list_actions().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(list).unwrap_or_default(),
    ))
}

async fn human_action_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let action = state.human.get_by_id_or_token(&id).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(action).unwrap_or_default(),
    ))
}

async fn human_action_complete_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<CompleteHumanActionBody>>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let mut login_result: Option<Value> = None;
    if let Some(Json(payload)) = body {
        if let Some(incoming_creds) = payload.credentials {
            let _ = state.auth.credentials().save(incoming_creds);
        }
        if let Some(code) = payload.sms_code {
            state.human.submit_ephemeral_sms_code(&id, code).await?;
            if let Some(ephemeral) = state.human.take_ephemeral_sms_code(&id).await {
                let _ = state.browser.submit_sms_in_browser(&ephemeral).await;
                if let Ok(Some(creds)) = state.auth.credentials().load() {
                    match state
                        .http
                        .submit_corporate_sms_login(&creds, &ephemeral)
                        .await
                    {
                        Ok(res) => {
                            tracing::info!(res = ?res, "Sinopec submit_corporate_sms_login response");
                            login_result = Some(res);
                        }
                        Err(e) => {
                            tracing::error!("submit_corporate_sms_login error: {e}");
                            login_result = Some(json!({
                                "error": e.to_string(),
                                "message": format!("提交短信验证码请求异常: {e}")
                            }));
                        }
                    }
                }
            }
        }
    }

    let completed = state.human.complete_action(&id).await?;
    let checked = state.service.check_auth().await?;

    // If the Human Action had a paused operation, update its phase so it can resume
    if let Some(op_id) = &completed.operation_id {
        if let Ok(Some(mut op)) = state.store.get_operation(op_id).await {
            op.phase = "RESUMED_AFTER_HUMAN_ACTION".to_string();
            op.state = if checked.verified {
                "READY_TO_RESUME"
            } else {
                "AUTH_UNVERIFIED"
            }
            .to_string();
            op.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = state.store.upsert_operation(&op).await;
        }
    }

    Ok(ApiEnvelope::success(json!({
        "human_action": completed,
        "auth_state": checked.state,
        "verified": checked.verified, "sms_login_response": login_result
    })))
}

async fn human_action_cancel_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let cancelled = state.human.cancel_action(&id).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(cancelled).unwrap_or_default(),
    ))
}

async fn settings_get_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let creds = state.auth.credentials().status()?;
    Ok(ApiEnvelope::success(json!({
        "username_configured": creds.username_configured,
        "password_configured": creds.password_configured,
        "phone_configured": creds.phone_configured,
        "corporate_tax_configured": creds.corporate_tax_configured,
        "corporate_id_configured": creds.corporate_id_configured,
        "corporate_holder_configured": creds.corporate_holder_configured,
        "master_card_configured": creds.master_card_configured,
        "account_mode": creds.account_mode,
        "masked_phone": creds.masked_phone,
        "masked_tax_code": creds.masked_tax_code,
        "masked_master_card": creds.masked_master_card,
        "province": creds.province,
        "auth_mode": state.config.auth_mode,
        "auto_login": state.config.auto_login,
        "max_auth_attempts": state.config.max_auth_attempts,
        "research_mode": state.config.research_mode,
        "demo_mode": state.config.demo_mode,
        "allow_auto_submit": state.config.allow_auto_submit,
        "steel_base_url": state.config.steel_base_url,
        "steel_cdp_url": state.config.steel_cdp_url,
        "scrapling_base_url": state.config.scrapling_base_url
    })))
}

async fn settings_put_credentials_handler(
    State(state): State<AppState>,
    Json(incoming): Json<StoredCredentials>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let saved = state.auth.credentials().save(incoming)?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(saved).unwrap_or_default(),
    ))
}

async fn settings_delete_credentials_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    state.auth.credentials().delete()?;
    let status = state.auth.credentials().status()?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(status).unwrap_or_default(),
    ))
}

async fn research_status_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let endpoints = state.store.list_research_endpoints().await?;
    let confirmed = endpoints.iter().filter(|e| e.status == "CONFIRMED").count();
    let hypothesis = endpoints
        .iter()
        .filter(|e| e.status == "HYPOTHESIS")
        .count();
    let unknown = endpoints.iter().filter(|e| e.status == "UNKNOWN").count();
    let auth = state.service.auth_status().await?;
    let snapshot = state
        .recorder
        .snapshot(auth.state.as_str(), confirmed, hypothesis, unknown)
        .await;
    Ok(ApiEnvelope::success(
        serde_json::to_value(snapshot).unwrap_or_default(),
    ))
}

async fn research_endpoints_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let endpoints = state.store.list_research_endpoints().await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(endpoints).unwrap_or_default(),
    ))
}

async fn auth_send_sms_handler(
    State(state): State<AppState>,
    body: Option<Json<StoredCredentials>>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    if let Some(Json(incoming)) = body {
        let _ = state.auth.credentials().save(incoming)?;
    }
    let creds = state.auth.credentials().load()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::CredentialsRequired,
            "No corporate credentials stored",
        )
    })?;
    let browser_fill = state
        .browser
        .auto_fill_and_send_sms_in_browser(&creds, false)
        .await
        .ok();
    let http_res = state
        .http
        .trigger_corporate_sms_with_auto_captcha(&creds)
        .await?;
    Ok(ApiEnvelope::success(json!({
        "captcha_expression": http_res.get("captcha_expression"),
        "captcha_answer": http_res.get("captcha_answer"),
        "sms_response": http_res.get("sms_response"),
        "steel_browser": browser_fill
    })))
}

async fn root_redirect_handler(State(state): State<AppState>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if let Ok(list) = state.human.list_actions().await {
        if let Some(first) = list.first() {
            return axum::response::Redirect::temporary(&format!("/human/{}", first.token))
                .into_response();
        }
    }
    axum::response::Redirect::temporary("/api/v1/health").into_response()
}

#[derive(Debug, Deserialize)]
pub struct CreateLiveInvoiceBody {
    pub start_date: String,
    pub end_date: String,
    pub mail: Option<String>,
}

async fn recharges_handler(
    State(state): State<AppState>,
    Query(q): Query<TransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.unwrap_or_else(|| "2026-01-01".to_string());
    let end = q.end_date.unwrap_or_else(|| "2026-09-25".to_string());
    let balance = state.http.probe_corporate_balance().await.ok().flatten();
    let recharges = state.http.query_live_recharges(&start, &end).await?;
    Ok(ApiEnvelope::success(json!({
        "start_date": start,
        "end_date": end,
        "card_balance": balance.as_ref().map(|b| &b.primary_card),
        "recharges": recharges
    })))
}

async fn invoice_create_live_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateLiveInvoiceBody>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let mail = req.mail.as_deref().unwrap_or("invoice@example.com");
    let windows = crate::sinopec::parser::SinopecParser::split_date_windows(
        &req.start_date,
        &req.end_date,
        360,
    )?;
    let mut results = Vec::new();
    for (w_start, w_end) in windows {
        let r = state
            .http
            .create_live_electronic_invoice(&w_start, &w_end, mail)
            .await?;
        results.push(r);
    }
    Ok(ApiEnvelope::success(json!({
        "windows_submitted": results.len(),
        "results": results
    })))
}

async fn allocations_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    let breakdown = state.http.query_allocation_breakdown().await?;
    Ok(ApiEnvelope::success(breakdown))
}

async fn browser_prewarm_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = std::time::Instant::now();
    let snap = state
        .browser
        .navigate("https://www.sinopecsales.com/default_corp.html")
        .await?;
    let elapsed = start.elapsed().as_millis();
    Ok(ApiEnvelope::success(json!({
        "prewarmed": true,
        "session_id": snap.session_id,
        "url": snap.url,
        "title": snap.title,
        "cookie_count": snap.sinopec_cookie_count,
        "elapsed_ms": elapsed
    })))
}
