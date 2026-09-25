use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::browser::BrowserDriver;
use crate::sinopec::models::{CreateInvoiceRequest, InvoicePreviewRequest};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub async fn mcp_get_handler(State(state): State<AppState>) -> impl IntoResponse {
    let tools = build_tool_definitions(state.config.research_mode);
    Json(json!({
        "name": "sinopec-agent-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "streamable-http",
        "research_mode": state.config.research_mode,
        "tool_count": tools.len()
    }))
}

pub async fn mcp_post_handler(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Response {
    let id = req.id.clone();
    match req.method.as_str() {
        "notifications/initialized" | "initialized" => StatusCode::ACCEPTED.into_response(),
        "initialize" => Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "sinopec-server",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": { "listChanged": false }
                }
            })),
            error: None,
        })
        .into_response(),
        "tools/list" => {
            let tools = build_tool_definitions(state.config.research_mode);
            Json(JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({ "tools": tools })),
                error: None,
            })
            .into_response()
        }
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let payload = dispatch_tool(&state, tool_name, args).await;
            Json(JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                    }],
                    "structuredContent": payload
                })),
                error: None,
            })
            .into_response()
        }
        _ => Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({
                "code": -32601,
                "message": format!("Method not found: {}", req.method)
            })),
        })
        .into_response(),
    }
}

async fn dispatch_tool(state: &AppState, name: &str, args: Value) -> Value {
    match name {
        "sinopec_auth_status" => match state.service.auth_status().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_auth_ensure" => match state.service.ensure_auth(None).await {
            Ok(v) => {
                if let Some(ha) = &v.active_human_action {
                    json!({
                        "status": "human_action_required",
                        "reason": ha.reason,
                        "message": ha.message,
                        "human_action_url": ha.human_action_url,
                        "viewer_url": ha.viewer_url,
                        "operation_id": ha.operation_id,
                        "can_resume": true
                    })
                } else {
                    json!({ "status": "ok", "data": v })
                }
            }
            Err(e) => format_mcp_error(e),
        },
        "sinopec_open_human_action" => {
            let reason = args.get("reason").and_then(|v| v.as_str());
            let message = args.get("message").and_then(|v| v.as_str());
            match state
                .service
                .open_human_action_for_user(reason, message)
                .await
            {
                Ok(ha) => json!({
                    "status": "human_action_required",
                    "reason": ha.reason,
                    "message": ha.message,
                    "human_action_url": ha.human_action_url,
                    "viewer_url": ha.viewer_url,
                    "operation_id": ha.operation_id,
                    "can_resume": true
                }),
                Err(e) => format_mcp_error(e),
            }
        }
        "sinopec_account_info" => match state.service.account_info().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_list_cards" => match state.service.list_cards().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_invoice_capabilities" => match state.service.detect_invoice_capabilities().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_invoice_quota" => match state.service.invoice_quota().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_invoice_preview" => {
            let req = InvoicePreviewRequest {
                start_date: args
                    .get("start_date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2026-09-01")
                    .to_string(),
                end_date: args
                    .get("end_date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2026-09-30")
                    .to_string(),
                card_id: args
                    .get("card_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                invoice_type: args
                    .get("invoice_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            match state.service.preview_invoice(req).await {
                Ok(v) => json!({ "status": "ok", "data": v }),
                Err(e) => format_mcp_error(e),
            }
        }
        "sinopec_invoice_create" => {
            let req = CreateInvoiceRequest {
                start_date: args
                    .get("start_date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2026-09-01")
                    .to_string(),
                end_date: args
                    .get("end_date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2026-09-30")
                    .to_string(),
                card_id: args
                    .get("card_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                invoice_type: args
                    .get("invoice_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                idempotency_key: args
                    .get("idempotency_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                confirmed: args
                    .get("confirmed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            };
            match state.service.create_invoice(req).await {
                Ok(v) => json!({ "status": "ok", "data": v }),
                Err(e) => format_mcp_error(e),
            }
        }
        "sinopec_invoice_list" => match state.service.list_invoices().await {
            Ok(v) => json!({ "status": "ok", "data": v }),
            Err(e) => format_mcp_error(e),
        },
        "sinopec_invoice_download" => {
            let id = args
                .get("id")
                .or_else(|| args.get("invoice_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("inv_202608_001");
            match state.service.download_invoice(id).await {
                Ok(v) => json!({ "status": "ok", "data": v }),
                Err(e) => format_mcp_error(e),
            }
        }
        "sinopec_research_status" if state.config.research_mode => {
            let endpoints = state
                .store
                .list_research_endpoints()
                .await
                .unwrap_or_default();
            let confirmed = endpoints.iter().filter(|e| e.status == "CONFIRMED").count();
            let hypothesis = endpoints
                .iter()
                .filter(|e| e.status == "HYPOTHESIS")
                .count();
            let unknown = endpoints.iter().filter(|e| e.status == "UNKNOWN").count();
            let auth = state
                .service
                .auth_status()
                .await
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|_| "UNKNOWN".to_string());
            let snap = state
                .recorder
                .snapshot(&auth, confirmed, hypothesis, unknown)
                .await;
            json!({ "status": "ok", "data": snap })
        }
        "sinopec_browser_open" if state.config.research_mode => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://www.sinopecsales.com/default_corp.html");
            match state.browser.navigate(url).await {
                Ok(snap) => {
                    state.recorder.update_page(&snap.url, &snap.title).await;
                    json!({ "status": "ok", "data": snap })
                }
                Err(e) => format_mcp_error(e),
            }
        }
        "sinopec_network_capture" if state.config.research_mode => {
            let _ = state.browser.start_network_capture().await;
            let auth = state
                .service
                .auth_status()
                .await
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|_| "UNKNOWN".to_string());
            let snap = state.recorder.snapshot(&auth, 7, 3, 1).await;
            json!({ "status": "ok", "recent_xhr": snap.recent_xhr })
        }
        "sinopec_research_snapshot" if state.config.research_mode => {
            let endpoints = state
                .store
                .list_research_endpoints()
                .await
                .unwrap_or_default();
            json!({ "status": "ok", "endpoints": endpoints })
        }
        _ => json!({
            "status": "error",
            "message": format!("Unknown or disabled MCP tool: {name}")
        }),
    }
}

fn format_mcp_error(err: crate::error::AppError) -> Value {
    if let Some(ha) = err.human_action {
        json!({
            "status": "human_action_required",
            "reason": err.code,
            "message": err.message,
            "human_action_url": ha.human_action_url,
            "viewer_url": ha.viewer_url,
            "operation_id": ha.operation_id,
            "can_resume": true
        })
    } else {
        json!({
            "status": "error",
            "code": err.code,
            "message": err.message
        })
    }
}

pub fn build_tool_definitions(research_mode: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "sinopec_auth_status",
            "description": "Query Sinopec authentication state machine, account mode (personal/corporate), and configured credential status.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_auth_ensure",
            "description": "Ensure Sinopec session is authenticated; syncs Steel Browser cookies or opens a recoverable Human Action if captcha/SMS is required.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_open_human_action",
            "description": "Create a Human Action URL and open Steel Browser on sinopecsales.com for manual login, SMS verification, or Human Demonstration.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "LOGIN, SMS_CODE, CAPTCHA, or LIVE_INVOICE_CONFIRM" },
                    "message": { "type": "string", "description": "Instruction message shown to the user" }
                }
            }
        }),
        json!({
            "name": "sinopec_account_info",
            "description": "Get current Sinopec account profile (Personal vs Corporate, customer code, masked tax code, invoice attribute).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_list_cards",
            "description": "List bound Sinopec fuel cards with masked card numbers (****1234), card type (entity/electronic), master/vice status, and balances.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_invoice_capabilities",
            "description": "Detect InvoiceCapabilities for the current Sinopec account and master card.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_invoice_quota",
            "description": "Query available corporate invoice quota amount in CNY.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_invoice_preview",
            "description": "Read-only preview of eligible Sinopec transactions, total amount (exact Decimal), capabilities, and blocking reasons for a date range.",
            "inputSchema": {
                "type": "object",
                "required": ["start_date", "end_date"],
                "properties": {
                    "start_date": { "type": "string", "description": "YYYY-MM-DD" },
                    "end_date": { "type": "string", "description": "YYYY-MM-DD" },
                    "card_id": { "type": "string", "description": "Optional card ID or masked suffix" },
                    "invoice_type": { "type": "string", "description": "Optional invoice type" }
                }
            }
        }),
        json!({
            "name": "sinopec_invoice_create",
            "description": "Submit Sinopec electronic invoice request with idempotency protection, auth re-verification, and Human Action recovery.",
            "inputSchema": {
                "type": "object",
                "required": ["start_date", "end_date"],
                "properties": {
                    "start_date": { "type": "string", "description": "YYYY-MM-DD" },
                    "end_date": { "type": "string", "description": "YYYY-MM-DD" },
                    "card_id": { "type": "string" },
                    "invoice_type": { "type": "string" },
                    "idempotency_key": { "type": "string" },
                    "confirmed": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "sinopec_invoice_list",
            "description": "List issued Sinopec electronic invoices and their download status.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "sinopec_invoice_download",
            "description": "Download official Sinopec electronic invoice PDF/OFD to /data/downloads/YYYY/MM/ with SHA-256 verification.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string", "description": "Invoice ID or invoice number" }
                }
            }
        }),
    ];

    if research_mode {
        tools.extend([
            json!({
                "name": "sinopec_research_status",
                "description": "[Research Mode Only] Inspect live research status, confirmed/unknown endpoints, and timeline.",
                "inputSchema": { "type": "object", "properties": {} }
            }),
            json!({
                "name": "sinopec_browser_open",
                "description": "[Research Mode Only] Navigate Steel Browser to a target sinopecsales.com URL and inspect DOM/cookies.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "url": { "type": "string" } }
                }
            }),
            json!({
                "name": "sinopec_network_capture",
                "description": "[Research Mode Only] Capture and list recent redacted XHR/fetch requests.",
                "inputSchema": { "type": "object", "properties": {} }
            }),
            json!({
                "name": "sinopec_research_snapshot",
                "description": "[Research Mode Only] Return the protocol endpoint inventory from SQLite research_state.",
                "inputSchema": { "type": "object", "properties": {} }
            }),
        ]);
    }
    tools
}
