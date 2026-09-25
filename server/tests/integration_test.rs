use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sinopec_server::{
    api::routes::build_router,
    config::{AppConfig, AuthMode},
    research::Redactor,
    sinopec::{
        models::{AccountMode, InvoicePreviewRequest},
        parser::SinopecParser,
    },
    state::AppState,
};
use tower::ServiceExt;
use uuid::Uuid;

fn test_config(research_mode: bool) -> AppConfig {
    let dir = std::env::temp_dir().join(format!("sinopec_test_{}", Uuid::new_v4().simple()));
    AppConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        public_base_url: "http://127.0.0.1:8788".to_string(),
        data_dir: dir,
        api_token: None,
        auth_mode: AuthMode::Hybrid,
        auto_login: true,
        max_auth_attempts: 3,
        research_mode,
        demo_mode: false,
        allow_auto_submit: false,
        steel_base_url: "http://127.0.0.1:13000".to_string(),
        steel_cdp_url: "ws://127.0.0.1:19223".to_string(),
        scrapling_base_url: None,
        sinopec_base_url: "http://127.0.0.1:19999".to_string(),
    }
}

#[test]
fn test_redactor_masks_cards_phones_and_secrets() {
    assert_eq!(
        Redactor::mask_card_number("1000111200000008816"),
        "****8816"
    );
    assert_eq!(Redactor::mask_phone("13812345678"), "138****5678");
    assert_eq!(
        Redactor::mask_id_or_tax("91120116MA06ABCDEF"),
        "911***********CDEF"
    );

    let raw = json!({
        "cardNo": "1000111200000008816",
        "mobile": "13987654321",
        "smsYzm": "839201",
        "password": "my_secret_password"
    });
    let redacted = Redactor::redact_json(&raw);
    assert_eq!(redacted["cardNo"], "****8816");
    assert_eq!(redacted["mobile"], "139****4321");
    assert_eq!(redacted["smsYzm"], "<REDACTED>");
    assert_eq!(redacted["password"], "<REDACTED>");
}

#[test]
fn test_parser_fen_to_decimal_and_date_windows() {
    let raw: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/account.json")).unwrap();
    let (acc, card) = SinopecParser::parse_balance_response(&raw).unwrap();
    assert_eq!(acc.account_mode, AccountMode::Corporate);
    assert_eq!(acc.customer_type_label, "单位多用户");
    assert_eq!(card.masked_card_no, "****8816");
    assert_eq!(card.province_code, "12");
    assert_eq!(card.province_name, "天津市");
    assert_eq!(card.balance, "1265.50");
    assert_eq!(card.reserve_balance, "3580.00");
    assert_eq!(card.invoice_attribute, "special_vat");

    let windows = SinopecParser::split_date_windows("2026-08-01", "2026-10-15", 31).unwrap();
    assert_eq!(windows.len(), 3);
    assert_eq!(
        windows[0],
        ("2026-08-01".to_string(), "2026-08-31".to_string())
    );
    assert_eq!(
        windows[1],
        ("2026-09-01".to_string(), "2026-10-01".to_string())
    );
    assert_eq!(
        windows[2],
        ("2026-10-02".to_string(), "2026-10-15".to_string())
    );
}

#[tokio::test]
async fn test_rest_and_mcp_and_human_ui_end_to_end() {
    let cfg = test_config(false);
    let data_dir = cfg.data_dir.clone();
    let state = AppState::initialize(cfg).await.unwrap();
    let app = build_router(state.clone());

    // 1. Save credentials via PUT /api/v1/settings/credentials and verify GET /api/v1/settings never leaks password
    let put_req = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/credentials")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "account_mode": "corporate",
                "username": "corp_admin",
                "password": "SuperSecretPassword123!",
                "phone": "13812345678",
                "tax_code": "91120116MA06ABCDEF",
                "id_number": "120101199001011234",
                "holder_name": "张三"
            })
            .to_string(),
        ))
        .unwrap();
    let put_resp = app.clone().oneshot(put_req).await.unwrap();
    assert_eq!(put_resp.status(), StatusCode::OK);

    let get_settings_req = Request::builder()
        .method("GET")
        .uri("/api/v1/settings")
        .body(Body::empty())
        .unwrap();
    let get_settings_resp = app.clone().oneshot(get_settings_req).await.unwrap();
    let bytes = axum::body::to_bytes(get_settings_resp.into_body(), 1024 * 64)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("SuperSecretPassword123!"));
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["data"]["password_configured"], true);
    assert_eq!(parsed["data"]["corporate_tax_configured"], true);

    // 2. Preview September 2026 invoices via service (exact Decimal arithmetic: 385.00 + 462.50 + 733.00 = 1580.50)
    let preview = state
        .service
        .preview_invoice(InvoicePreviewRequest {
            start_date: "2026-09-01".to_string(),
            end_date: "2026-09-30".to_string(),
            card_id: None,
            invoice_type: None,
        })
        .await
        .unwrap();
    assert_eq!(preview.transaction_count, 3);
    assert_eq!(preview.total_amount, "1580.50");
    assert_eq!(preview.total_amount_fen, 158050);

    // 3. Test MCP tools/list in production mode (research_mode = false -> 11 tools, no research tools)
    let mcp_list_req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        ))
        .unwrap();
    let mcp_resp = app.clone().oneshot(mcp_list_req).await.unwrap();
    let mcp_bytes = axum::body::to_bytes(mcp_resp.into_body(), 1024 * 64)
        .await
        .unwrap();
    let mcp_json: Value = serde_json::from_slice(&mcp_bytes).unwrap();
    let tools = mcp_json["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 11);

    // 4. Test Human Action creation and built-in /human/{token} HTML UI
    let ha = state
        .service
        .open_human_action_for_user(Some("SMS_CODE"), Some("测试短信验证"))
        .await
        .unwrap();
    let human_ui_req = Request::builder()
        .method("GET")
        .uri(format!("/human/{}", ha.token))
        .body(Body::empty())
        .unwrap();
    let human_ui_resp = app.clone().oneshot(human_ui_req).await.unwrap();
    assert_eq!(human_ui_resp.status(), StatusCode::OK);

    // 5. Test Invoice Download with SHA-256 verification
    let dl = state
        .service
        .download_invoice("inv_202608_001")
        .await
        .unwrap();
    assert_eq!(dl.sha256.len(), 64);
    assert!(std::path::Path::new(&dl.path).exists());

    let _ = std::fs::remove_dir_all(data_dir);
}
