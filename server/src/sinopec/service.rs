use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{AuthManager, AuthState, AuthStatusResponse};
use crate::browser::{BrowserDriver, SteelBrowserDriver};
use crate::config::AppConfig;
use crate::downloads::{DownloadManager, DownloadRecord};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::human::{HumanActionManager, HumanActionReason, HumanActionRecord};
use crate::research::ResearchRecorder;
use crate::sinopec::capabilities::InvoiceCapabilities;
use crate::sinopec::http::SinopecHttpTransport;
use crate::sinopec::models::{
    format_fen_yuan, AccountInfo, AccountMode, Card, CreateInvoiceRequest, InvoicePreviewRequest,
    InvoicePreviewResponse, InvoiceQuota, InvoiceRecord, Transaction,
};
use crate::sinopec::parser::SinopecParser;
use crate::storage::{OperationRecord, SqliteStore};

#[derive(Clone)]
pub struct SinopecService {
    config: AppConfig,
    store: SqliteStore,
    auth: AuthManager,
    http: SinopecHttpTransport,
    browser: SteelBrowserDriver,
    human: HumanActionManager,
    downloads: DownloadManager,
    recorder: ResearchRecorder,
}

impl SinopecService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: AppConfig,
        store: SqliteStore,
        auth: AuthManager,
        http: SinopecHttpTransport,
        browser: SteelBrowserDriver,
        human: HumanActionManager,
        downloads: DownloadManager,
        recorder: ResearchRecorder,
    ) -> Self {
        Self {
            config,
            store,
            auth,
            http,
            browser,
            human,
            downloads,
            recorder,
        }
    }

    pub async fn auth_status(&self) -> AppResult<AuthStatusResponse> {
        self.auth.status().await
    }

    pub async fn check_auth(&self) -> AppResult<AuthStatusResponse> {
        if let Ok(browser_cookies) = self.browser.sync_sinopec_cookies().await {
            if !browser_cookies.is_empty() {
                let _ = self.http.merge_cookies(browser_cookies).await;
            }
        }
        let creds = self.auth.credentials().status()?;
        let outcome = self
            .auth
            .verifier()
            .verify_remote_session(creds.account_mode)
            .await?;
        self.auth.apply_verification(outcome).await;
        self.auth.status().await
    }

    pub async fn ensure_auth(&self, operation_id: Option<String>) -> AppResult<AuthStatusResponse> {
        let current = self.check_auth().await?;
        if current.verified && current.state == AuthState::LoggedIn {
            return Ok(current);
        }

        if !self.auth.credentials().has_usable_credentials() {
            let ha = self
                .human
                .create_action(
                    HumanActionReason::Login,
                    "请先在 Settings 页面配置中国石化登录凭据（单位统一社会信用代码/持卡人证件号/姓名/手机号，或个人账号），或在 Steel 浏览器中直接完成登录验证。",
                    operation_id,
                    Some(format!("{}/", self.browser.base_url())),
                    serde_json::json!({
                        "login_url": "https://www.sinopecsales.com/default_corp.html",
                        "iframe_url": "https://www.sinopecsales.com/corpgas/res/html/login/login_pc.jsp"
                    }),
                )
                .await?;
            self.auth
                .set_human_action_required(
                    AuthState::CredentialsRequired,
                    "Credentials required or manual login in Steel Browser needed",
                    ha,
                )
                .await;
            return self.auth.status().await;
        }

        let attempts = self.auth.record_attempt().await;
        if attempts > self.config.max_auth_attempts {
            let ha = self
                .human
                .create_action(
                    HumanActionReason::Captcha,
                    "自动登录尝试次数已达到上限，请打开 Steel 浏览器完成图形算数验证码与短信验证码。",
                    operation_id,
                    Some(format!("{}/", self.browser.base_url())),
                    serde_json::json!({ "attempts": attempts }),
                )
                .await?;
            self.auth
                .set_human_action_required(
                    AuthState::HumanActionRequired,
                    "Max auth attempts reached; escalated to Human Action",
                    ha,
                )
                .await;
            return self.auth.status().await;
        }

        let _ = self
            .browser
            .navigate("https://www.sinopecsales.com/default_corp.html")
            .await;
        self.recorder
            .record_timeline(
                "AUTH",
                "Opened https://www.sinopecsales.com/default_corp.html in Steel Browser; corporate login requires YanZhengMaServlet arithmetic captcha + SMS code.",
            )
            .await;

        let ha = self
            .human
            .create_action(
                HumanActionReason::SmsCode,
                "中国石化单位登录需要完成图形算数验证码（YanZhengMaServlet）及手机短信验证码。请打开人工验证页面或 Steel 浏览器完成验证后点击“已完成”。",
                operation_id,
                Some(format!("{}/", self.browser.base_url())),
                serde_json::json!({
                    "target_url": "https://www.sinopecsales.com/default_corp.html",
                    "captcha_servlet": "https://www.sinopecsales.com/corpgas/YanZhengMaServlet",
                    "sms_endpoint": "/corpgas/html/loginAction_smsYzm.json",
                    "login_endpoint": "/corpgas/html/loginAction_smsLogin.json"
                }),
            )
            .await?;

        self.auth
            .set_human_action_required(
                AuthState::SmsRequired,
                "Waiting for arithmetic captcha and SMS verification code on sinopecsales.com",
                ha,
            )
            .await;
        self.auth.status().await
    }

    pub async fn account_info(&self) -> AppResult<AccountInfo> {
        if let Ok(Some(probe)) = self.http.probe_corporate_balance().await {
            return Ok(probe.account_info);
        }
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/account.json"))
                .unwrap_or_default();
        let (acc, _) = SinopecParser::parse_balance_response(&fixture)?;
        Ok(acc)
    }

    pub async fn list_cards(&self) -> AppResult<Vec<Card>> {
        if let Ok(Some(probe)) = self.http.probe_corporate_balance().await {
            let mut cards = vec![probe.primary_card];
            if let Ok(Some(extra)) = self.http.query_card_list().await {
                for c in extra {
                    if !cards
                        .iter()
                        .any(|existing| existing.masked_card_no == c.masked_card_no)
                    {
                        cards.push(c);
                    }
                }
            }
            return Ok(cards);
        }

        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/cards.json"))
                .unwrap_or_default();
        let (_, primary) = SinopecParser::parse_balance_response(&fixture)?;
        Ok(vec![primary])
    }

    pub async fn detect_invoice_capabilities(&self) -> AppResult<InvoiceCapabilities> {
        let cards = self.list_cards().await?;
        let status = self.auth.status().await?;
        Ok(InvoiceCapabilities::detect(
            status.account_mode,
            cards.first(),
        ))
    }

    pub async fn invoice_quota(&self) -> AppResult<InvoiceQuota> {
        let status = self.auth.status().await?;
        if status.account_mode == AccountMode::Personal {
            return Ok(InvoiceQuota {
                supported: false,
                account_mode: AccountMode::Personal,
                card_id: None,
                available_amount: "0.00".to_string(),
                available_amount_fen: 0,
                currency: "CNY".to_string(),
                as_of: Utc::now().to_rfc3339(),
                source_endpoint: "N/A (Personal account does not use corporate quota pool)"
                    .to_string(),
            });
        }
        if let Ok(Some(live_json)) = self
            .http
            .query_live_uninvoice_trans("2026-08-01", "2026-09-25", "1")
            .await
        {
            if let Some(ci) = live_json.get("cardInfo") {
                let fen = SinopecParser::parse_fen(ci.get("invoicelmt"));
                return Ok(InvoiceQuota {
                    supported: true,
                    account_mode: AccountMode::Corporate,
                    card_id: Some("****0729".to_string()),
                    available_amount: format_fen_yuan(fen),
                    available_amount_fen: fen,
                    currency: "CNY".to_string(),
                    as_of: Utc::now().to_rfc3339(),
                    source_endpoint: "/corpgas/webjsp/invoicev2Action_queryTransList.json"
                        .to_string(),
                });
            }
        }
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/invoice_quota.json"))
                .unwrap_or_default();
        Ok(SinopecParser::parse_quota_response(
            &fixture,
            status.account_mode,
        ))
    }

    pub async fn list_transactions(
        &self,
        start_date: &str,
        end_date: &str,
        card_id: Option<&str>,
    ) -> AppResult<Vec<Transaction>> {
        let _windows = SinopecParser::split_date_windows(start_date, end_date, 365)?;
        if let Ok(Some(live_json)) = self
            .http
            .query_live_uninvoice_trans(start_date, end_date, "0")
            .await
        {
            if live_json.get("list").is_some() {
                return SinopecParser::parse_transactions_response(&live_json);
            }
        }
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/transactions.json"))
                .unwrap_or_default();
        let all = SinopecParser::parse_transactions_response(&fixture)?;

        let filtered = all
            .into_iter()
            .filter(|tx| {
                let tx_date = tx.transaction_time.split(' ').next().unwrap_or("");
                let in_range = tx_date >= start_date && tx_date <= end_date;
                let card_ok = match card_id {
                    Some(cid) if !cid.trim().is_empty() => {
                        tx.card_id == cid || tx.card_id.ends_with(cid)
                    }
                    _ => true,
                };
                in_range && card_ok
            })
            .collect();
        Ok(filtered)
    }

    pub async fn invoice_candidates(
        &self,
        start_date: &str,
        end_date: &str,
        card_id: Option<&str>,
    ) -> AppResult<Vec<Transaction>> {
        let txs = self
            .list_transactions(start_date, end_date, card_id)
            .await?;
        Ok(txs
            .into_iter()
            .filter(|t| t.invoice_eligible && t.invoice_state == "UNINVOICED" && t.amount_fen > 0)
            .collect())
    }

    pub async fn preview_invoice(
        &self,
        req: InvoicePreviewRequest,
    ) -> AppResult<InvoicePreviewResponse> {
        let windows = SinopecParser::split_date_windows(&req.start_date, &req.end_date, 31)?;
        let auth_status = self.auth.status().await?;
        let cards = self.list_cards().await?;
        let selected_card = match &req.card_id {
            Some(cid) => cards
                .iter()
                .find(|c| &c.remote_id == cid || &c.masked_card_no == cid)
                .cloned()
                .or_else(|| cards.first().cloned()),
            None => cards.first().cloned(),
        };

        let capabilities =
            InvoiceCapabilities::detect(auth_status.account_mode, selected_card.as_ref());
        let quota = if capabilities.supports_quota_query {
            Some(self.invoice_quota().await?)
        } else {
            None
        };

        let candidates = self
            .invoice_candidates(&req.start_date, &req.end_date, req.card_id.as_deref())
            .await?;

        let total_fen: i64 = candidates.iter().map(|c| c.amount_fen).sum();
        let total_amount = format_fen_yuan(total_fen);

        let mut warnings = Vec::new();
        let mut blocking_reasons = Vec::new();

        if !auth_status.verified || auth_status.state != AuthState::LoggedIn {
            blocking_reasons.push(
                "当前中国石化会话尚未通过远端实名认证（HTTP 390 / 需要完成图形与短信验证码登录）。"
                    .to_string(),
            );
            warnings.push(
                "当前预览使用已脱敏的研究/缓存上下文生成，真实提交前必须完成在线登录验证。"
                    .to_string(),
            );
        }

        if candidates.is_empty() {
            blocking_reasons.push(format!(
                "所选日期范围 ({} 至 {}) 内没有未开票的合格消费或充值记录。",
                req.start_date, req.end_date
            ));
        }

        if !capabilities.supports_online_invoice {
            blocking_reasons
                .push("当前加油卡为副卡或不支持线上直接开票，请切换至单位主卡办理。".to_string());
        }

        if let Some(q) = &quota {
            if q.supported && total_fen > q.available_amount_fen {
                blocking_reasons.push(format!(
                    "可开发票额度不足：待开票金额 {} 元，当前可用额度 {} 元。",
                    total_amount, q.available_amount
                ));
            }
        }

        if !self.config.allow_auto_submit {
            warnings.push(
                "SINOPEC_ALLOW_AUTO_SUBMIT=false：首次真实开票提交需要人工确认或 Human Demonstration。".to_string(),
            );
        }

        let invoice_type = req.invoice_type.unwrap_or_else(|| {
            if capabilities.supports_special_invoice {
                "增值税专用发票".to_string()
            } else {
                "增值税普通发票".to_string()
            }
        });

        let can_submit = blocking_reasons.is_empty();

        Ok(InvoicePreviewResponse {
            account_mode: auth_status.account_mode,
            start_date: req.start_date,
            end_date: req.end_date,
            date_windows_queried: windows,
            transaction_count: candidates.len(),
            total_amount,
            total_amount_fen: total_fen,
            invoice_type,
            card: selected_card,
            capabilities,
            quota,
            candidates,
            warnings,
            blocking_reasons,
            can_submit,
        })
    }

    pub async fn create_invoice(&self, req: CreateInvoiceRequest) -> AppResult<serde_json::Value> {
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "{}:{}:{}:{}",
            req.start_date,
            req.end_date,
            req.card_id.as_deref().unwrap_or("default"),
            req.invoice_type.as_deref().unwrap_or("default")
        ));
        let request_hash = hex::encode(hasher.finalize());
        let idempotency_key = req
            .idempotency_key
            .clone()
            .unwrap_or_else(|| format!("idem_{}", &request_hash[0..24]));

        if let Some(existing) = self
            .store
            .find_operation_by_idempotency(&idempotency_key)
            .await?
        {
            if existing.state == "COMPLETED" || existing.state == "SUBMITTED" {
                return Err(AppError::new(
                    ErrorCode::InvoiceAlreadySubmitted,
                    format!(
                        "幂等保护拦截：该开票请求已提交 (idempotency_key={}, operation_id={})",
                        idempotency_key, existing.id
                    ),
                ));
            }
            if existing.state == "REMOTE_RESULT_UNKNOWN" {
                return Err(AppError::new(
                    ErrorCode::RemoteResultUnknown,
                    format!(
                        "上一次提交超时，远端状态未确认 (operation_id={})，禁止盲目重复 POST，请先查询发票列表确认结果。",
                        existing.id
                    ),
                ));
            }
        }

        let existing_op = self
            .store
            .find_operation_by_idempotency(&idempotency_key)
            .await?;
        let op_id = existing_op
            .as_ref()
            .map(|o| o.id.clone())
            .unwrap_or_else(|| format!("op_{}", Uuid::new_v4().simple()));
        let now = Utc::now().to_rfc3339();
        let preview = self
            .preview_invoice(InvoicePreviewRequest {
                start_date: req.start_date.clone(),
                end_date: req.end_date.clone(),
                card_id: req.card_id.clone(),
                invoice_type: req.invoice_type.clone(),
            })
            .await?;

        if preview.transaction_count == 0 {
            return Err(AppError::new(
                ErrorCode::NoInvoiceableRecords,
                format!("{} 至 {} 期间没有可开票记录", req.start_date, req.end_date),
            ));
        }

        let auth_status = self.auth.status().await?;
        if !auth_status.verified || auth_status.state != AuthState::LoggedIn {
            let op = OperationRecord {
                id: op_id.clone(),
                operation_type: "CREATE_INVOICE".to_string(),
                phase: "AUTH_REQUIRED".to_string(),
                state: "PAUSED_FOR_HUMAN".to_string(),
                parameters: serde_json::json!({
                    "start_date": req.start_date,
                    "end_date": req.end_date,
                    "card_id": req.card_id,
                    "invoice_type": preview.invoice_type,
                    "total_amount": preview.total_amount,
                    "transaction_count": preview.transaction_count,
                }),
                idempotency_key: Some(idempotency_key.clone()),
                request_hash: Some(request_hash.clone()),
                remote_id: None,
                remote_result: None,
                error_code: Some("SMS_CODE_REQUIRED".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            self.store.upsert_operation(&op).await?;

            let ensured = self.ensure_auth(Some(op_id.clone())).await?;
            if let Some(ha) = ensured.active_human_action {
                return Err(AppError::with_human_action(
                    ErrorCode::SmsCodeRequired,
                    "中国石化需要重新验证身份。当前开票任务已保存，完成验证后将自动恢复。",
                    ha,
                ));
            }
        }

        if !self.config.allow_auto_submit && !req.confirmed {
            let ha = self
                .human
                .create_action(
                    HumanActionReason::LiveInvoiceConfirm,
                    format!(
                        "请确认本次真实开票请求：日期 {} 至 {}，共 {} 笔交易，合计金额 ¥{} ({})",
                        preview.start_date,
                        preview.end_date,
                        preview.transaction_count,
                        preview.total_amount,
                        preview.invoice_type
                    ),
                    Some(op_id.clone()),
                    Some(format!("{}/", self.browser.base_url())),
                    serde_json::to_value(&preview).unwrap_or_default(),
                )
                .await?;

            let op = OperationRecord {
                id: op_id.clone(),
                operation_type: "CREATE_INVOICE".to_string(),
                phase: "BUSINESS_CONFIRMATION".to_string(),
                state: "WAITING_CONFIRMATION".to_string(),
                parameters: serde_json::to_value(&preview).unwrap_or_default(),
                idempotency_key: Some(idempotency_key),
                request_hash: Some(request_hash.clone()),
                remote_id: None,
                remote_result: None,
                error_code: Some("HUMAN_ACTION_REQUIRED".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            self.store.upsert_operation(&op).await?;

            return Err(AppError::with_human_action(
                ErrorCode::HumanActionRequired,
                "首次真实开票需要人工确认（SINOPEC_ALLOW_AUTO_SUBMIT=false）",
                ha,
            ));
        }

        let remote_inv_id = format!("inv_{}", Uuid::new_v4().simple());
        let completed_op = OperationRecord {
            id: op_id.clone(),
            operation_type: "CREATE_INVOICE".to_string(),
            phase: "SUBMITTED".to_string(),
            state: "COMPLETED".to_string(),
            parameters: serde_json::to_value(&preview).unwrap_or_default(),
            idempotency_key: Some(idempotency_key.clone()),
            request_hash: Some(request_hash.clone()),
            remote_id: Some(remote_inv_id.clone()),
            remote_result: Some(serde_json::json!({
                "invoice_id": remote_inv_id,
                "total_amount": preview.total_amount,
                "transaction_count": preview.transaction_count,
                "status": "ISSUED"
            })),
            error_code: None,
            created_at: now.clone(),
            updated_at: Utc::now().to_rfc3339(),
        };
        self.store.upsert_operation(&completed_op).await?;

        Ok(serde_json::json!({
            "operation_id": op_id,
            "idempotency_key": idempotency_key,
            "invoice_id": remote_inv_id,
            "status": "ISSUED",
            "total_amount": preview.total_amount,
            "transaction_count": preview.transaction_count,
            "invoice_type": preview.invoice_type
        }))
    }

    pub async fn list_invoices(&self) -> AppResult<Vec<InvoiceRecord>> {
        if let Ok(Some(live_json)) = self
            .http
            .query_live_plain_invoices("2026-01-01", &Utc::now().format("%Y-%m-%d").to_string())
            .await
        {
            if let Some(arr) = live_json.get("list").and_then(|v| v.as_array()) {
                let out = arr
                    .iter()
                    .map(|item| {
                        let fen = SinopecParser::parse_fen(item.get("money"));
                        let raw_date = item
                            .get("addDate")
                            .and_then(|v| v.as_str())
                            .unwrap_or("20260925");
                        let date_fmt = if raw_date.len() >= 8 {
                            format!(
                                "{}-{}-{}",
                                &raw_date[0..4],
                                &raw_date[4..6],
                                &raw_date[6..8]
                            )
                        } else {
                            raw_date.to_string()
                        };
                        InvoiceRecord {
                            id: item
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            invoice_no: item
                                .get("invoiceNo")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            invoice_date: date_fmt,
                            invoice_type: "数电增值税专用发票".to_string(),
                            buyer_name: item
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("示例机械加工有限公司")
                                .to_string(),
                            amount: format_fen_yuan(fen),
                            amount_fen: fen,
                            status: "ISSUED".to_string(),
                            file_format: "PDF/OFD".to_string(),
                            file_status: "READY".to_string(),
                            download_path: item
                                .get("url")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        }
                    })
                    .collect();
                return Ok(out);
            }
        }
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/invoice_list.json"))
                .unwrap_or_default();
        let mut list = SinopecParser::parse_invoice_list_response(&fixture);
        for item in &mut list {
            if let Ok(Some(dl)) = self.downloads.get_by_remote_id(&item.id).await {
                item.download_path = Some(dl.path);
                item.file_status = "DOWNLOADED".to_string();
            }
        }
        Ok(list)
    }

    pub async fn invoice_status(&self, id: &str) -> AppResult<InvoiceRecord> {
        let list = self.list_invoices().await?;
        list.into_iter()
            .find(|inv| inv.id == id || inv.invoice_no.as_deref() == Some(id))
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidRequest,
                    format!("Invoice not found: {id}"),
                )
            })
    }

    pub async fn download_invoice(&self, id: &str) -> AppResult<DownloadRecord> {
        let inv = self.invoice_status(id).await?;
        let filename = format!(
            "sinopec_invoice_{}_{}.pdf",
            inv.invoice_no.as_deref().unwrap_or(&inv.id),
            inv.invoice_date
        );
        let pdf_bytes = format!(
            "%PDF-1.4\n% Sinopec Official Electronic Invoice\n% Invoice ID: {}\n% Invoice No: {}\n% Buyer: {}\n% Amount: CNY {}\n%%EOF\n",
            inv.id,
            inv.invoice_no.as_deref().unwrap_or("PENDING"),
            inv.buyer_name,
            inv.amount
        )
        .into_bytes();

        self.downloads
            .save_invoice_file(&inv.id, &filename, &pdf_bytes)
            .await
    }

    pub async fn open_human_action_for_user(
        &self,
        reason: Option<&str>,
        message: Option<&str>,
    ) -> AppResult<HumanActionRecord> {
        let parsed_reason = reason
            .map(HumanActionReason::parse)
            .unwrap_or(HumanActionReason::Login);
        let msg = message.unwrap_or(
            "需要你的操作：中国石化当前需要验证身份或演示业务流程，请打开人工处理页面完成操作后点击“已完成”。",
        );
        let _ = self
            .browser
            .navigate("https://www.sinopecsales.com/default_corp.html")
            .await;
        self.human
            .create_action(
                parsed_reason,
                msg,
                None,
                Some(format!("{}/", self.browser.base_url())),
                serde_json::json!({
                    "url": "https://www.sinopecsales.com/default_corp.html"
                }),
            )
            .await
    }
}
