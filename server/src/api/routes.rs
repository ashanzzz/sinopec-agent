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
        .route("/auth/sms/send", post(auth_send_sms_handler))
        .route("/auth/sms/login", post(auth_direct_sms_login_handler))
        .route("/auth/sms-login", post(auth_direct_sms_login_handler))
        .route("/auth/login", post(admin_login_handler))
        .route("/auth/session", get(admin_session_handler))
        .route("/auth/logout", post(admin_logout_handler))
        .route("/account", get(account_handler))
        .route("/cards", get(cards_handler))
        .route("/invoice-capabilities", get(invoice_capabilities_handler))
        .route("/invoice-quota", get(invoice_quota_handler))
        .route("/transactions", get(transactions_handler))
        .route("/recharges", get(recharges_handler))
        .route("/allocations", get(allocations_handler))
        .route("/allocations/logs", get(allocations_logs_handler))
        .route("/local/sync-status", get(local_sync_status_handler))
        .route("/local/sync", post(local_sync_trigger_handler))
        .route("/local/quancun", get(local_quancun_list_handler))
        .route("/local/invoices", get(local_invoices_list_handler))
        .route("/local/allocations", get(local_allocations_handler))
        .route(
            "/local/allocations/logs",
            get(local_allocations_logs_handler),
        )
        .route("/local/transactions", get(local_transactions_handler))
        .route(
            "/local/financial-summary",
            get(local_financial_summary_handler),
        )
        .route(
            "/cards/{card_no}/transactions",
            get(card_transactions_handler),
        )
        .route("/browser/prewarm", post(browser_prewarm_handler))
        .route("/browser/screenshot", get(browser_screenshot_handler))
        .route("/invoices/create-live", post(invoice_create_live_handler))
        .route(
            "/invoices/preview",
            post(invoice_preview_handler).get(invoice_preview_get_handler),
        )
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

#[derive(Debug, Deserialize)]
pub struct InvoicePreviewQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub card_id: Option<String>,
    pub invoice_type: Option<String>,
}

async fn invoice_preview_get_handler(
    State(state): State<AppState>,
    Query(q): Query<InvoicePreviewQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let req = InvoicePreviewRequest {
        start_date: q.start_date.unwrap_or_else(|| "2026-09-01".to_string()),
        end_date: q.end_date.unwrap_or_else(|| "2026-09-30".to_string()),
        card_id: q.card_id,
        invoice_type: q.invoice_type,
    };
    let preview = state.service.preview_invoice(req).await?;
    Ok(ApiEnvelope::success(
        serde_json::to_value(preview).unwrap_or_default(),
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

#[derive(Debug, Deserialize)]
pub struct DirectSmsLoginBody {
    pub sms_code: String,
    pub credentials: Option<StoredCredentials>,
}

async fn auth_direct_sms_login_handler(
    State(state): State<AppState>,
    Json(payload): Json<DirectSmsLoginBody>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    if let Some(incoming) = payload.credentials {
        let _ = state.auth.credentials().save(incoming)?;
    }
    let creds = state.auth.credentials().load()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::CredentialsRequired,
            "No corporate credentials stored",
        )
    })?;
    let res = state
        .http
        .submit_corporate_sms_login(&creds, &payload.sms_code)
        .await?;
    let _ = state.service.check_auth().await;
    Ok(ApiEnvelope::success(res))
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
    let mail = req.mail.as_deref().unwrap_or("335356119@qq.com");
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
    let (spanning, total_trans, total_money) = if results.len() == 1 {
        let r0 = &results[0];
        (
            r0.get("spanning_cutover")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            r0.get("total_trans_count")
                .and_then(|v| v.as_i64())
                .or_else(|| r0.get("trans_count").and_then(|v| v.as_i64())),
            r0.get("total_money")
                .or_else(|| r0.get("money"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        )
    } else {
        (false, None, None)
    };
    Ok(ApiEnvelope::success(json!({
        "windows_submitted": results.len(),
        "spanning_cutover": spanning,
        "total_trans_count": total_trans,
        "total_money": total_money,
        "results": results
    })))
}

async fn allocations_handler(State(state): State<AppState>) -> AppResult<Json<ApiEnvelope<Value>>> {
    match state.http.query_allocation_breakdown().await {
        Ok(breakdown) => {
            let _ = state.store.set_kv("sync:allocations", &breakdown).await;
            Ok(ApiEnvelope::success(breakdown))
        }
        Err(_) => {
            if let Ok(Some(cached)) = state.store.get_kv("sync:allocations").await {
                Ok(ApiEnvelope::success(cached))
            } else {
                Err(AppError::new(
                    ErrorCode::SinopecUnavailable,
                    "无法连接中石化网厅且本地尚无资金大盘缓存",
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AllocationLogsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub card_no: Option<String>,
}

async fn allocations_logs_handler(
    State(state): State<AppState>,
    Query(q): Query<AllocationLogsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start_ref = q.start_date.as_deref();
    let end_ref = q.end_date.as_deref();
    let card_ref = q.card_no.as_deref();

    // 1. Try local SQLite query first (fast, reliable, cross-month friendly)
    if let Ok(local_list) = state
        .store
        .list_local_allocations(start_ref, end_ref, card_ref)
        .await
    {
        if !local_list.is_empty() {
            return Ok(ApiEnvelope::success(json!({
                "count": local_list.len(),
                "list": local_list,
                "source": "local_sqlite"
            })));
        }
    }

    // 2. Fallback to online query if local has no data
    let start = q.start_date.unwrap_or_else(|| "2026-01-01".to_string());
    let end = q.end_date.unwrap_or_else(|| "2026-12-31".to_string());
    let master_card_no = state
        .http
        .resolve_default_card_no()
        .await
        .unwrap_or_else(|_| "1000111200006330729".to_string());
    let raw_logs = state
        .http
        .query_yufenpei_logs(&master_card_no, &start, &end)
        .await?;
    let _ = state
        .store
        .save_local_allocations(&master_card_no, &raw_logs)
        .await;

    let filtered: Vec<Value> = if let Some(target_card) = &q.card_no {
        if target_card == "all" || target_card.is_empty() {
            raw_logs
        } else {
            raw_logs
                .into_iter()
                .filter(|a| a.get("cardNo").and_then(|v| v.as_str()) == Some(target_card.as_str()))
                .collect()
        }
    } else {
        raw_logs
    };
    Ok(ApiEnvelope::success(json!({
        "master_card_no": master_card_no,
        "start_date": start,
        "end_date": end,
        "count": filtered.len(),
        "list": filtered,
        "source": "sinopec_live"
    })))
}

#[derive(Debug, Deserialize)]
pub struct CardTransactionsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub card_no: Option<String>,
    pub tra_type: Option<String>,
}

async fn card_transactions_handler(
    State(state): State<AppState>,
    Path(card_no): Path<String>,
    Query(q): Query<CardTransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start_ref = q.start_date.as_deref();
    let end_ref = q.end_date.as_deref();
    let tra_type_str = q.tra_type.as_deref().unwrap_or("消费");

    // 1. Try local SQLite query first (fast, reliable, offline-safe)
    if tra_type_str == "圈存" {
        if let Ok(local_list) = state
            .store
            .list_local_quancun(start_ref, end_ref, Some(&card_no))
            .await
        {
            if !local_list.is_empty() {
                let mut total_fen: i64 = 0;
                for it in &local_list {
                    total_fen += crate::sinopec::parser::SinopecParser::parse_fen(it.get("amount"));
                }
                return Ok(ApiEnvelope::success(json!({
                    "card_no": card_no,
                    "count": local_list.len(),
                    "total_amount": crate::sinopec::models::format_fen_yuan(total_fen),
                    "list": local_list,
                    "source": "local_sqlite"
                })));
            }
        }
    } else {
        if let Ok(local_list) = state
            .store
            .list_local_transactions(start_ref, end_ref, Some(&card_no))
            .await
        {
            if !local_list.is_empty() {
                let mut total_fen: i64 = 0;
                for it in &local_list {
                    total_fen += crate::sinopec::parser::SinopecParser::parse_fen(it.get("amount"));
                }
                return Ok(ApiEnvelope::success(json!({
                    "card_no": card_no,
                    "count": local_list.len(),
                    "total_amount": crate::sinopec::models::format_fen_yuan(total_fen),
                    "list": local_list,
                    "source": "local_sqlite"
                })));
            }
        }
    }

    // 2. Fallback to online query if local has no data
    let start = q.start_date.unwrap_or_else(|| "2026-01-01".to_string());
    let end = q.end_date.unwrap_or_else(|| "2026-12-31".to_string());
    let raw_logs = if card_no == "all" {
        let master = state
            .http
            .resolve_default_card_no()
            .await
            .unwrap_or_else(|_| "1000111200006330729".to_string());
        let vice_cards = state
            .http
            .query_vice_cards(&master)
            .await
            .unwrap_or_default();
        let mut card_list = vec![master];
        for vc in vice_cards {
            if let Some(cno) = vc.get("cardNo").and_then(|v| v.as_str()) {
                if !card_list.contains(&cno.to_string()) {
                    card_list.push(cno.to_string());
                }
            }
        }
        let mut all_logs = Vec::new();
        for c in &card_list {
            if let Ok(mut logs) = state
                .http
                .query_card_transaction_logs(c, &start, &end)
                .await
            {
                for item in &mut logs {
                    if item.get("cardNo").is_none() {
                        item["cardNo"] = serde_json::json!(c);
                    }
                }
                all_logs.extend(logs);
            }
        }
        all_logs.sort_by(|a, b| {
            let t_a = a.get("opeTime").and_then(|v| v.as_str()).unwrap_or("");
            let t_b = b.get("opeTime").and_then(|v| v.as_str()).unwrap_or("");
            t_b.cmp(t_a)
        });
        all_logs
    } else {
        let mut logs = state
            .http
            .query_card_transaction_logs(&card_no, &start, &end)
            .await?;
        for item in &mut logs {
            if item.get("cardNo").is_none() {
                item["cardNo"] = serde_json::json!(card_no);
            }
        }
        logs
    };

    let filtered: Vec<Value> = if let Some(tt) = &q.tra_type {
        if tt == "all" || tt.is_empty() {
            raw_logs
        } else {
            raw_logs
                .into_iter()
                .filter(|item| {
                    item.get("traName")
                        .and_then(|v| v.as_str())
                        .map(|s| s.contains(tt))
                        .unwrap_or(false)
                })
                .collect()
        }
    } else {
        raw_logs
    };

    let total_fen: i64 = filtered
        .iter()
        .map(|item| crate::sinopec::parser::SinopecParser::parse_fen(item.get("amount")))
        .sum();
    let total_amount = crate::sinopec::models::format_fen_yuan(total_fen);

    Ok(ApiEnvelope::success(json!({
        "card_no": card_no,
        "start_date": start,
        "end_date": end,
        "tra_type": q.tra_type.unwrap_or_else(|| "all".to_string()),
        "count": filtered.len(),
        "total_amount": total_amount,
        "total_fen": total_fen,
        "list": filtered
    })))
}

async fn browser_screenshot_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let url = state.browser.get_url().await.unwrap_or_default();
    let title = state.browser.get_title().await.unwrap_or_default();
    let b64 = state.browser.screenshot().await?.unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    Ok(ApiEnvelope::success(json!({
        "url": url,
        "title": title,
        "screenshot_png_base64": b64,
        "captured_at": now
    })))
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

#[derive(Debug, Deserialize)]
pub struct AdminLoginBody {
    pub username: String,
    pub password: String,
}

async fn admin_login_handler(
    State(state): State<AppState>,
    Json(body): Json<AdminLoginBody>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let ok = state
        .store
        .verify_admin_user(&body.username, &body.password)
        .await?;
    if ok {
        let token = format!("sinopec_token_{}", uuid::Uuid::new_v4().simple());
        let _ = state
            .store
            .set_kv(
                &format!("session:{}", token),
                &json!({
                    "username": body.username,
                    "created_at": chrono::Utc::now().to_rfc3339()
                }),
            )
            .await;
        Ok(ApiEnvelope::success(json!({
            "token": token,
            "username": body.username,
            "authenticated": true
        })))
    } else {
        Err(AppError::new(
            ErrorCode::LoginRequired,
            "用户名或密码错误，请核对后重试".to_string(),
        ))
    }
}

async fn admin_session_handler(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let token = req
        .headers()
        .get("X-Sinopec-Auth-Token")
        .or_else(|| req.headers().get(axum::http::header::AUTHORIZATION))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).trim());

    if let Some(tok) = token {
        if let Ok(Some(sess)) = state.store.get_kv(&format!("session:{}", tok)).await {
            return Ok(ApiEnvelope::success(json!({
                "authenticated": true,
                "username": sess.get("username").and_then(|v| v.as_str()).unwrap_or("ashan"),
                "created_at": sess.get("created_at")
            })));
        }
    }

    Ok(ApiEnvelope::success(json!({
        "authenticated": false
    })))
}

async fn admin_logout_handler(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let token = req
        .headers()
        .get("X-Sinopec-Auth-Token")
        .or_else(|| req.headers().get(axum::http::header::AUTHORIZATION))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).trim());

    if let Some(tok) = token {
        let _ = state
            .store
            .set_kv(&format!("session:{}", tok), &json!({ "logged_out": true }))
            .await;
    }
    Ok(ApiEnvelope::success(json!({ "success": true })))
}

async fn local_sync_status_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let meta = state.store.get_kv("sync:meta").await?.unwrap_or_else(|| {
        json!({
            "last_synced_at": "暂未同步",
            "status": "pending"
        })
    });
    let auth = state.service.auth_status().await.ok();
    let remote_online = auth.map(|a| a.verified).unwrap_or(false);

    Ok(ApiEnvelope::success(json!({
        "last_synced_at": meta.get("last_synced_at").unwrap_or(&json!("暂未同步")),
        "remote_online": remote_online,
        "meta": meta
    })))
}

async fn local_sync_trigger_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let auth = state.service.auth_status().await?;
    let beijing =
        chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
    let now_str = beijing.format("%Y-%m-%d %H:%M:%S").to_string();

    if !auth.verified {
        let last_synced = state
            .store
            .get_kv("sync:meta")
            .await?
            .and_then(|v| v.get("last_synced_at").cloned());
        return Ok(ApiEnvelope::success(json!({
            "ok": false,
            "remote_online": false,
            "last_synced_at": last_synced.unwrap_or(json!("未同步")),
            "message": "中石化网厅会话已离线（需要短信验证码重新登录）。当前已加载本地 SQLite 历史数据快照。"
        })));
    }

    let breakdown = state.http.query_allocation_breakdown().await?;
    state.store.set_kv("sync:allocations", &breakdown).await?;

    let mut all_quancun = Vec::new();
    let mut all_transactions = Vec::new();
    if let Some(cards) = breakdown.get("cards_ledger").and_then(|v| v.as_object()) {
        for (card_no, c_data) in cards {
            let alias = c_data
                .get("card_alias")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let holder = c_data.get("holder").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(q_recs) = c_data.get("quancun_records").and_then(|v| v.as_array()) {
                for q in q_recs {
                    let mut item = q.clone();
                    if let Some(obj) = item.as_object_mut() {
                        obj.insert("card_no".to_string(), json!(card_no));
                        obj.insert("card_alias".to_string(), json!(alias));
                        obj.insert("holder_name".to_string(), json!(holder));
                    }
                    all_quancun.push(item);
                }
            }
            if let Some(tx_recs) = c_data.get("transaction_records").and_then(|v| v.as_array()) {
                for t in tx_recs {
                    let mut item = t.clone();
                    if let Some(obj) = item.as_object_mut() {
                        obj.insert("card_no".to_string(), json!(card_no));
                        obj.insert("card_alias".to_string(), json!(alias));
                        obj.insert("holder_name".to_string(), json!(holder));
                    }
                    let tra_name = item.get("traName").and_then(|v| v.as_str()).unwrap_or("");
                    if tra_name == "圈存" {
                        all_quancun.push(item);
                    } else {
                        all_transactions.push(item);
                    }
                }
            }
        }
    }
    let quancun_count = state.store.save_local_quancun(&all_quancun).await?;
    let tx_count = state
        .store
        .save_local_transactions(&all_transactions)
        .await?;

    let today = beijing.format("%Y-%m-%d").to_string();
    let start_date = "2026-01-01";

    let master_card_no = state
        .http
        .resolve_default_card_no()
        .await
        .unwrap_or_else(|_| "1000111200006330729".to_string());
    let raw_allocs = state
        .http
        .query_yufenpei_logs(&master_card_no, start_date, &today)
        .await
        .unwrap_or_default();
    let alloc_count = state
        .store
        .save_local_allocations(&master_card_no, &raw_allocs)
        .await
        .unwrap_or(0);

    let invoices_val = state
        .http
        .query_live_plain_invoices(start_date, &today)
        .await
        .ok()
        .flatten();
    let invoice_count = if let Some(val) = invoices_val {
        if let Some(list) = val.get("list").and_then(|v| v.as_array()) {
            state.store.save_local_invoices(list).await?
        } else {
            0
        }
    } else {
        0
    };

    let sync_meta = json!({
        "last_synced_at": now_str,
        "remote_online": true,
        "allocations_count": alloc_count,
        "quancun_records_count": quancun_count,
        "transactions_count": tx_count,
        "invoice_records_count": invoice_count,
        "status": "success"
    });
    state.store.set_kv("sync:meta", &sync_meta).await?;

    Ok(ApiEnvelope::success(json!({
        "ok": true,
        "remote_online": true,
        "last_synced_at": now_str,
        "allocations_synced": alloc_count,
        "quancun_synced": quancun_count,
        "transactions_synced": tx_count,
        "invoices_synced": invoice_count,
        "message": format!(
            "同步成功！已将 {} 笔预分配划拨、{} 笔圈存、{} 笔消费明细与 {} 笔发票全量落盘至本地 SQLite 数据库。",
            alloc_count, quancun_count, tx_count, invoice_count
        )
    })))
}

async fn local_allocations_logs_handler(
    State(state): State<AppState>,
    Query(q): Query<AllocationLogsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.as_deref();
    let end = q.end_date.as_deref();
    let card = q.card_no.as_deref();
    let list = state.store.list_local_allocations(start, end, card).await?;
    Ok(ApiEnvelope::success(json!({
        "count": list.len(),
        "list": list
    })))
}

async fn local_transactions_handler(
    State(state): State<AppState>,
    Query(q): Query<CardTransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.as_deref();
    let end = q.end_date.as_deref();
    let card = q.card_no.as_deref();
    let list = state
        .store
        .list_local_transactions(start, end, card)
        .await?;
    Ok(ApiEnvelope::success(json!({
        "count": list.len(),
        "list": list
    })))
}

async fn local_financial_summary_handler(
    State(state): State<AppState>,
    Query(q): Query<TransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.as_deref();
    let end = q.end_date.as_deref();
    let summary = state.store.get_financial_summary(start, end).await?;
    Ok(ApiEnvelope::success(summary))
}

async fn local_quancun_list_handler(
    State(state): State<AppState>,
    Query(q): Query<CardTransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.as_deref();
    let end = q.end_date.as_deref();
    let list = state.store.list_local_quancun(start, end, None).await?;
    Ok(ApiEnvelope::success(json!({
        "count": list.len(),
        "list": list
    })))
}

async fn local_invoices_list_handler(
    State(state): State<AppState>,
    Query(q): Query<TransactionsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let start = q.start_date.as_deref();
    let end = q.end_date.as_deref();
    let list = state.store.list_local_invoices(start, end).await?;
    Ok(ApiEnvelope::success(json!({
        "count": list.len(),
        "list": list
    })))
}

async fn local_allocations_handler(
    State(state): State<AppState>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    if let Ok(Some(cached)) = state.store.get_kv("sync:allocations").await {
        return Ok(ApiEnvelope::success(cached));
    }
    let breakdown = state.http.query_allocation_breakdown().await?;
    Ok(ApiEnvelope::success(breakdown))
}
