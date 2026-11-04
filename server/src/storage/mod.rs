use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::path::Path;
use std::str::FromStr;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub id: String,
    pub operation_type: String,
    pub phase: String,
    pub state: String,
    pub parameters: serde_json::Value,
    pub idempotency_key: Option<String>,
    pub request_hash: Option<String>,
    pub remote_id: Option<String>,
    pub remote_result: Option<serde_json::Value>,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchEndpointRecord {
    pub id: String,
    pub endpoint_name: String,
    pub account_mode: String,
    pub method: String,
    pub path: String,
    pub status: String,
    pub notes: String,
    pub last_verified_at: String,
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(db_path: &Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let opts = SqliteConnectOptions::from_str(&db_url)
            .map_err(|e| {
                crate::error::AppError::new(crate::error::ErrorCode::InternalError, e.to_string())
            })?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        let schema = include_str!("../../migrations/0001_initial_schema.sql");
        for stmt in schema.split(';') {
            let trimmed = stmt.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&pool).await?;
            }
        }

        let store = Self { pool };
        store.seed_default_research_state().await?;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn set_kv(&self, key: &str, value: &serde_json::Value) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let val_str = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        sqlx::query(
            "INSERT INTO kv (key, value_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(val_str)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_kv(&self, key: &str) -> AppResult<Option<serde_json::Value>> {
        let row = sqlx::query("SELECT value_json FROM kv WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|r| {
            let s: String = r.get("value_json");
            serde_json::from_str(&s).ok()
        }))
    }

    pub async fn upsert_operation(&self, op: &OperationRecord) -> AppResult<()> {
        let params_str = serde_json::to_string(&op.parameters).unwrap_or_else(|_| "{}".to_string());
        let remote_res_str = op
            .remote_result
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        sqlx::query(
            "INSERT INTO operations (
                id, operation_type, phase, state, parameters_json, idempotency_key,
                request_hash, remote_id, remote_result_json, error_code, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                phase = excluded.phase,
                state = excluded.state,
                parameters_json = excluded.parameters_json,
                remote_id = excluded.remote_id,
                remote_result_json = excluded.remote_result_json,
                error_code = excluded.error_code,
                updated_at = excluded.updated_at",
        )
        .bind(&op.id)
        .bind(&op.operation_type)
        .bind(&op.phase)
        .bind(&op.state)
        .bind(params_str)
        .bind(&op.idempotency_key)
        .bind(&op.request_hash)
        .bind(&op.remote_id)
        .bind(remote_res_str)
        .bind(&op.error_code)
        .bind(&op.created_at)
        .bind(&op.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_operation(&self, id: &str) -> AppResult<Option<OperationRecord>> {
        let row = sqlx::query("SELECT * FROM operations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| Self::row_to_operation(&r)))
    }

    pub async fn find_operation_by_idempotency(
        &self,
        idempotency_key: &str,
    ) -> AppResult<Option<OperationRecord>> {
        let row = sqlx::query("SELECT * FROM operations WHERE idempotency_key = ?")
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| Self::row_to_operation(&r)))
    }

    pub async fn latest_operation(&self) -> AppResult<Option<OperationRecord>> {
        let row = sqlx::query("SELECT * FROM operations ORDER BY updated_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| Self::row_to_operation(&r)))
    }

    pub async fn list_research_endpoints(&self) -> AppResult<Vec<ResearchEndpointRecord>> {
        let rows = sqlx::query("SELECT * FROM research_state ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| ResearchEndpointRecord {
                id: r.get("id"),
                endpoint_name: r.get("endpoint_name"),
                account_mode: r.get("account_mode"),
                method: r.get("method"),
                path: r.get("path"),
                status: r.get("status"),
                notes: r.get("notes"),
                last_verified_at: r.get("last_verified_at"),
            })
            .collect())
    }

    async fn seed_default_research_state(&self) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let endpoints = [
            (
                "ep_auth_status_corp",
                "Corporate Auth Session Check",
                "corporate",
                "POST",
                "/corpgas/html/memberLoginAction_logInOrOut.json",
                "CONFIRMED",
                "Returns memberAccount, cardNum, ssoServerRootURL",
            ),
            (
                "ep_comp_type",
                "Company Account Mode Detector",
                "corporate",
                "POST",
                "/corpgas/html/memberLoginAction_getcompType.json",
                "CONFIRMED",
                "Returns compType, openIve, loginstate (Y/N)",
            ),
            (
                "ep_corp_captcha",
                "Corporate Image Captcha",
                "corporate",
                "GET",
                "/corpgas/YanZhengMaServlet",
                "CONFIRMED",
                "Arithmetic image captcha for corporate login",
            ),
            (
                "ep_corp_sms_send",
                "Corporate Login SMS Trigger",
                "corporate",
                "POST",
                "/corpgas/html/loginAction_smsYzm.json",
                "CONFIRMED",
                "Sends SMS code after tax/idno/name/mobile/check verification",
            ),
            (
                "ep_corp_sms_login",
                "Corporate Login Submit",
                "corporate",
                "POST",
                "/corpgas/html/loginAction_smsLogin.json",
                "CONFIRMED",
                "Establishes JSESSIONID and MYSERVERID_corpgas session cookies",
            ),
            (
                "ep_card_balance",
                "Master Card & Account Info Query",
                "corporate",
                "POST",
                "/corpgas/webjsp/billQueryAction_queryBalance.json",
                "CONFIRMED",
                "Returns HTTP 390 when unauthenticated; returns cardMember + cardInfo in integer Fen when authenticated",
            ),
            (
                "ep_card_list",
                "Bound Fuel Card List Query",
                "corporate",
                "POST",
                "/corpgas/webjsp/memberOilCardAction_queryMyOilCardList.json",
                "CONFIRMED",
                "Returns cards array and maplist keyed by 19-digit card number",
            ),
            (
                "ep_bill_detail",
                "Transaction & Consumption Detail",
                "corporate",
                "GET/POST",
                "/corpgas/webjsp/query/billDetail.jsp",
                "HYPOTHESIS",
                "Page entry confirmed; JSON action payload requires authenticated capture",
            ),
            (
                "ep_invoice_quota",
                "Corporate Invoice Quota Query",
                "corporate",
                "GET/POST",
                "/corpgas/webjsp/invoice/queryAmount.jsp",
                "HYPOTHESIS",
                "Page entry confirmed; JSON action payload requires authenticated capture",
            ),
            (
                "ep_invoice_create_v2",
                "Electronic Invoice Create V2",
                "corporate",
                "GET/POST",
                "/corpgas/webjsp/invoicev2/createInvoice.jsp",
                "UNKNOWN",
                "Page entry confirmed; requires Human Demonstration before real submit",
            ),
            (
                "ep_invoice_bill_query",
                "Electronic Invoice Bill & Download",
                "corporate",
                "GET/POST",
                "/corpgas/webjsp/invoicev2/queryInvoiceBill.jsp",
                "HYPOTHESIS",
                "Page entry confirmed; JSON list & file download requires authenticated capture",
            ),
        ];

        for (id, name, mode, method, path, status, notes) in endpoints {
            sqlx::query(
                "INSERT OR IGNORE INTO research_state
                 (id, endpoint_name, account_mode, method, path, status, notes, last_verified_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(mode)
            .bind(method)
            .bind(path)
            .bind(status)
            .bind(notes)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    fn row_to_operation(r: &sqlx::sqlite::SqliteRow) -> OperationRecord {
        let params_str: String = r.get("parameters_json");
        let remote_res_str: Option<String> = r.get("remote_result_json");
        OperationRecord {
            id: r.get("id"),
            operation_type: r.get("operation_type"),
            phase: r.get("phase"),
            state: r.get("state"),
            parameters: serde_json::from_str(&params_str).unwrap_or_else(|_| serde_json::json!({})),
            idempotency_key: r.get("idempotency_key"),
            request_hash: r.get("request_hash"),
            remote_id: r.get("remote_id"),
            remote_result: remote_res_str.and_then(|s| serde_json::from_str(&s).ok()),
            error_code: r.get("error_code"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }

    pub async fn seed_admin_user(&self, username: &str, password: &str) -> AppResult<()> {
        use sha2::{Digest, Sha256};
        let now = Utc::now().to_rfc3339();
        let salt = "sinopec_enterprise_salt_2026";
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", salt, password).as_bytes());
        let hash = hex::encode(hasher.finalize());

        sqlx::query(
            "INSERT INTO admin_users (username, password_hash, salt, created_at, last_login_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(username) DO UPDATE SET password_hash = excluded.password_hash, salt = excluded.salt",
        )
        .bind(username)
        .bind(&hash)
        .bind(salt)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn verify_admin_user(&self, username: &str, password: &str) -> AppResult<bool> {
        use sha2::{Digest, Sha256};
        let row = sqlx::query("SELECT password_hash, salt FROM admin_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        let Some(r) = row else {
            return Ok(false);
        };

        let stored_hash: String = r.get("password_hash");
        let salt: String = r.get("salt");

        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", salt, password).as_bytes());
        let computed_hash = hex::encode(hasher.finalize());

        if stored_hash == computed_hash {
            let now = Utc::now().to_rfc3339();
            let _ = sqlx::query("UPDATE admin_users SET last_login_at = ? WHERE username = ?")
                .bind(&now)
                .bind(username)
                .execute(&self.pool)
                .await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn save_local_quancun(&self, records: &[serde_json::Value]) -> AppResult<usize> {
        let now = Utc::now().to_rfc3339();
        let mut count = 0;
        for r in records {
            let card_no = r.get("card_no").and_then(|v| v.as_str()).unwrap_or("");
            let card_alias = r.get("card_alias").and_then(|v| v.as_str());
            let holder_name = r.get("holder_name").and_then(|v| v.as_str());
            let ope_time = r.get("opeTime").and_then(|v| v.as_str()).unwrap_or("");
            let amount_fen = r.get("amount").and_then(|v| v.as_str()).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
            let amount_yuan = crate::sinopec::models::format_fen_yuan(amount_fen);
            let balance_fen = r.get("balance").and_then(|v| v.as_str()).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
            let balance_yuan = crate::sinopec::models::format_fen_yuan(balance_fen);
            let node_tag = r.get("nodeTag").and_then(|v| v.as_str()).unwrap_or("").trim();
            let tra_name = r.get("traName").and_then(|v| v.as_str()).unwrap_or("圈存");
            let oil_name = r.get("oilName").and_then(|v| v.as_str());

            if card_no.is_empty() || ope_time.is_empty() {
                continue;
            }

            let res = sqlx::query(
                "INSERT INTO local_quancun_records 
                 (card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, balance_yuan, node_tag, tra_name, oil_name, synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(card_no, ope_time, amount_fen) DO UPDATE SET
                    balance_yuan = excluded.balance_yuan,
                    node_tag = excluded.node_tag,
                    card_alias = COALESCE(excluded.card_alias, local_quancun_records.card_alias),
                    synced_at = excluded.synced_at"
            )
            .bind(card_no)
            .bind(card_alias)
            .bind(holder_name)
            .bind(ope_time)
            .bind(amount_fen)
            .bind(&amount_yuan)
            .bind(&balance_yuan)
            .bind(node_tag)
            .bind(tra_name)
            .bind(oil_name)
            .bind(&now)
            .execute(&self.pool)
            .await;

            if res.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn list_local_quancun(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        card_no: Option<&str>,
    ) -> AppResult<Vec<serde_json::Value>> {
        let s = start_date.unwrap_or("");
        let e = end_date.unwrap_or("9999-99-99");
        let c = card_no.unwrap_or("all");

        let rows = sqlx::query(
            "SELECT card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, balance_yuan, node_tag, tra_name, oil_name, synced_at 
             FROM local_quancun_records 
             WHERE substr(ope_time, 1, 10) >= ? 
               AND substr(ope_time, 1, 10) <= ?
               AND (? = 'all' OR ? = '' OR card_no = ?)
             ORDER BY ope_time DESC"
        )
        .bind(s)
        .bind(e)
        .bind(c)
        .bind(c)
        .bind(c)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for r in rows {
            let c_no: String = r.get("card_no");
            let c_alias: Option<String> = r.get("card_alias");
            let h_name: Option<String> = r.get("holder_name");
            let o_time: String = r.get("ope_time");
            let a_fen: i64 = r.get("amount_fen");
            let a_yuan: String = r.get("amount_yuan");
            let b_yuan: String = r.get("balance_yuan");
            let n_tag: String = r.get("node_tag");
            let t_name: String = r.get("tra_name");
            let o_name: Option<String> = r.get("oil_name");
            let s_at: String = r.get("synced_at");

            out.push(serde_json::json!({
                "card_no": c_no,
                "card_alias": c_alias,
                "holder_name": h_name,
                "opeTime": o_time,
                "amount": a_fen.to_string(),
                "amount_yuan": a_yuan,
                "balance": b_yuan,
                "nodeTag": n_tag,
                "traName": t_name,
                "oilName": o_name.unwrap_or_default(),
                "synced_at": s_at
            }));
        }
        Ok(out)
    }

    pub async fn save_local_invoices(&self, records: &[serde_json::Value]) -> AppResult<usize> {
        let now = Utc::now().to_rfc3339();
        let mut count = 0;
        for r in records {
            let invoice_no = r.get("invoice_no").and_then(|v| v.as_str()).unwrap_or("");
            if invoice_no.is_empty() {
                continue;
            }
            let invoice_number = r.get("invoiceNumber").or_else(|| r.get("invoice_number")).and_then(|v| v.as_str());
            let einv_req_key = r.get("einvReqKey").or_else(|| r.get("einv_req_key")).and_then(|v| v.as_str());
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("天津祺富机械加工有限公司");
            let amount_fen = r.get("money_fen").or_else(|| r.get("amount_fen")).and_then(|v| v.as_i64()).unwrap_or(0);
            let amount_yuan = r.get("money").or_else(|| r.get("amount")).and_then(|v| v.as_str()).unwrap_or("0.00");
            let fplx = r.get("fplx").and_then(|v| v.as_str()).unwrap_or("02");
            let add_date = r.get("addDate").or_else(|| r.get("add_date")).and_then(|v| v.as_str()).unwrap_or("");
            let status = r.get("status").and_then(|v| v.as_i64()).unwrap_or(1);
            let url = r.get("url").and_then(|v| v.as_str());
            let start_date = r.get("start_date").and_then(|v| v.as_str());
            let end_date = r.get("end_date").and_then(|v| v.as_str());
            let system = r.get("system").and_then(|v| v.as_str());

            let res = sqlx::query(
                "INSERT INTO local_invoices 
                 (invoice_no, invoice_number, einv_req_key, title, amount_fen, amount_yuan, fplx, add_date, status, url, start_date, end_date, system, synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(invoice_no) DO UPDATE SET
                    invoice_number = COALESCE(excluded.invoice_number, local_invoices.invoice_number),
                    url = COALESCE(excluded.url, local_invoices.url),
                    status = excluded.status,
                    synced_at = excluded.synced_at"
            )
            .bind(invoice_no)
            .bind(invoice_number)
            .bind(einv_req_key)
            .bind(title)
            .bind(amount_fen)
            .bind(amount_yuan)
            .bind(fplx)
            .bind(add_date)
            .bind(status)
            .bind(url)
            .bind(start_date)
            .bind(end_date)
            .bind(system)
            .bind(&now)
            .execute(&self.pool)
            .await;

            if res.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn list_local_invoices(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<Vec<serde_json::Value>> {
        let s = start_date.unwrap_or("");
        let e = end_date.unwrap_or("9999-99-99");

        let rows = sqlx::query(
            "SELECT invoice_no, invoice_number, einv_req_key, title, amount_fen, amount_yuan, fplx, add_date, status, url, start_date, end_date, system, synced_at 
             FROM local_invoices 
             WHERE (start_date >= ? OR substr(add_date, 1, 10) >= ?)
               AND (end_date <= ? OR substr(add_date, 1, 10) <= ?)
             ORDER BY add_date DESC, id DESC"
        )
        .bind(s)
        .bind(s)
        .bind(e)
        .bind(e)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::json!({
                "invoice_no": r.get::<String, _>("invoice_no"),
                "invoiceNumber": r.get::<Option<String>, _>("invoice_number"),
                "einvReqKey": r.get::<Option<String>, _>("einv_req_key"),
                "title": r.get::<Option<String>, _>("title"),
                "amount_fen": r.get::<i64, _>("amount_fen"),
                "amount": r.get::<String, _>("amount_yuan"),
                "fplx": r.get::<Option<String>, _>("fplx"),
                "addDate": r.get::<String, _>("add_date"),
                "status": r.get::<Option<i64>, _>("status"),
                "url": r.get::<Option<String>, _>("url"),
                "start_date": r.get::<Option<String>, _>("start_date"),
                "end_date": r.get::<Option<String>, _>("end_date"),
                "system": r.get::<Option<String>, _>("system"),
                "synced_at": r.get::<String, _>("synced_at")
            }));
        }
        Ok(out)
    }

    pub async fn save_local_allocations(
        &self,
        master_card_no: &str,
        records: &[serde_json::Value],
    ) -> AppResult<usize> {
        let beijing = Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        let now = beijing.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut count = 0;

        for r in records {
            let card_no = r.get("cardNo").and_then(|v| v.as_str()).unwrap_or("");
            let ope_time = r.get("opeTime").and_then(|v| v.as_str()).unwrap_or("");
            let amount_fen = r
                .get("amount")
                .and_then(|v| {
                    if let Some(n) = v.as_i64() {
                        Some(n)
                    } else if let Some(s) = v.as_str() {
                        s.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let amount_yuan = crate::sinopec::models::format_fen_yuan(amount_fen);
            let holder_name = r
                .get("cardHolder")
                .and_then(|v| v.as_str())
                .unwrap_or("孟祥山")
                .trim();
            let node_name = r
                .get("nodeName")
                .and_then(|v| v.as_str())
                .unwrap_or("网厅划拨")
                .trim();
            let status_text = r
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("100% 已圈存入芯");

            let card_alias = match card_no {
                c if c.ends_with("8807") => "车间",
                c if c.ends_with("8459") => "老板",
                c if c.ends_with("9423") => "李佳宁",
                c if c.ends_with("0729") => "孟祥山",
                _ => "",
            };

            if card_no.is_empty() || ope_time.is_empty() {
                continue;
            }

            let res = sqlx::query(
                "INSERT INTO local_allocations 
                 (master_card_no, card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, node_name, status_text, synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(card_no, ope_time, amount_fen) DO UPDATE SET
                    node_name = excluded.node_name,
                    status_text = excluded.status_text,
                    synced_at = excluded.synced_at"
            )
            .bind(master_card_no)
            .bind(card_no)
            .bind(card_alias)
            .bind(holder_name)
            .bind(ope_time)
            .bind(amount_fen)
            .bind(&amount_yuan)
            .bind(node_name)
            .bind(status_text)
            .bind(&now)
            .execute(&self.pool)
            .await;

            if res.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn list_local_allocations(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        card_no: Option<&str>,
    ) -> AppResult<Vec<serde_json::Value>> {
        let s = start_date.unwrap_or("");
        let e = end_date.unwrap_or("9999-99-99");
        let c = card_no.unwrap_or("all");

        let rows = sqlx::query(
            "SELECT master_card_no, card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, node_name, status_text, synced_at 
             FROM local_allocations 
             WHERE substr(ope_time, 1, 10) >= ? 
               AND substr(ope_time, 1, 10) <= ?
               AND (? = 'all' OR ? = '' OR card_no = ?)
             ORDER BY ope_time DESC"
        )
        .bind(s)
        .bind(e)
        .bind(c)
        .bind(c)
        .bind(c)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::json!({
                "master_card_no": r.get::<String, _>("master_card_no"),
                "cardNo": r.get::<String, _>("card_no"),
                "card_alias": r.get::<Option<String>, _>("card_alias"),
                "cardHolder": r.get::<Option<String>, _>("holder_name"),
                "opeTime": r.get::<String, _>("ope_time"),
                "amount": r.get::<i64, _>("amount_fen").to_string(),
                "amount_yuan": r.get::<String, _>("amount_yuan"),
                "nodeName": r.get::<String, _>("node_name"),
                "status": r.get::<String, _>("status_text"),
                "synced_at": r.get::<String, _>("synced_at")
            }));
        }
        Ok(out)
    }

    pub async fn save_local_transactions(
        &self,
        records: &[serde_json::Value],
    ) -> AppResult<usize> {
        let beijing = Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        let now = beijing.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut count = 0;

        for r in records {
            let card_no = r.get("card_no").or_else(|| r.get("cardNo")).and_then(|v| v.as_str()).unwrap_or("");
            let ope_time = r.get("opeTime").and_then(|v| v.as_str()).unwrap_or("");
            let tra_name = r.get("traName").and_then(|v| v.as_str()).unwrap_or("消费");
            let amount_fen = r
                .get("amount")
                .and_then(|v| {
                    if let Some(n) = v.as_i64() {
                        Some(n)
                    } else if let Some(s) = v.as_str() {
                        s.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let amount_yuan = crate::sinopec::models::format_fen_yuan(amount_fen);
            let balance_fen = r
                .get("balance")
                .and_then(|v| {
                    if let Some(n) = v.as_i64() {
                        Some(n)
                    } else if let Some(s) = v.as_str() {
                        s.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let balance_yuan = crate::sinopec::models::format_fen_yuan(balance_fen);
            let node_tag = r.get("nodeTag").and_then(|v| v.as_str()).unwrap_or("").trim();
            let oil_name = r.get("oilName").and_then(|v| v.as_str());
            let price_yuan = r.get("price").and_then(|v| v.as_str());
            let volume_litre = r.get("volume").or_else(|| r.get("oilVol")).and_then(|v| v.as_str());
            let holder_name = r.get("holder_name").and_then(|v| v.as_str());

            let card_alias = match card_no {
                c if c.ends_with("8807") => "车间",
                c if c.ends_with("8459") => "老板",
                c if c.ends_with("9423") => "李佳宁",
                c if c.ends_with("0729") => "孟祥山",
                _ => "",
            };

            if card_no.is_empty() || ope_time.is_empty() {
                continue;
            }

            let res = sqlx::query(
                "INSERT INTO local_transactions 
                 (card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, balance_yuan, oil_name, price_yuan, volume_litre, node_tag, tra_name, synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(card_no, ope_time, amount_fen, balance_yuan) DO UPDATE SET
                    node_tag = excluded.node_tag,
                    oil_name = COALESCE(excluded.oil_name, local_transactions.oil_name),
                    price_yuan = COALESCE(excluded.price_yuan, local_transactions.price_yuan),
                    volume_litre = COALESCE(excluded.volume_litre, local_transactions.volume_litre),
                    synced_at = excluded.synced_at"
            )
            .bind(card_no)
            .bind(card_alias)
            .bind(holder_name)
            .bind(ope_time)
            .bind(amount_fen)
            .bind(&amount_yuan)
            .bind(&balance_yuan)
            .bind(oil_name)
            .bind(price_yuan)
            .bind(volume_litre)
            .bind(node_tag)
            .bind(tra_name)
            .bind(&now)
            .execute(&self.pool)
            .await;

            if res.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn list_local_transactions(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        card_no: Option<&str>,
    ) -> AppResult<Vec<serde_json::Value>> {
        let s = start_date.unwrap_or("");
        let e = end_date.unwrap_or("9999-99-99");
        let c = card_no.unwrap_or("all");

        let rows = sqlx::query(
            "SELECT card_no, card_alias, holder_name, ope_time, amount_fen, amount_yuan, balance_yuan, oil_name, price_yuan, volume_litre, node_tag, tra_name, synced_at 
             FROM local_transactions 
             WHERE substr(ope_time, 1, 10) >= ? 
               AND substr(ope_time, 1, 10) <= ?
               AND (? = 'all' OR ? = '' OR card_no = ?)
             ORDER BY ope_time DESC"
        )
        .bind(s)
        .bind(e)
        .bind(c)
        .bind(c)
        .bind(c)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::new();
        for r in rows {
            out.push(serde_json::json!({
                "card_no": r.get::<String, _>("card_no"),
                "card_alias": r.get::<Option<String>, _>("card_alias"),
                "holder_name": r.get::<Option<String>, _>("holder_name"),
                "opeTime": r.get::<String, _>("ope_time"),
                "amount": r.get::<i64, _>("amount_fen").to_string(),
                "amount_yuan": r.get::<String, _>("amount_yuan"),
                "balance": r.get::<String, _>("balance_yuan"),
                "oilName": r.get::<Option<String>, _>("oil_name").unwrap_or_default(),
                "price": r.get::<Option<String>, _>("price_yuan").unwrap_or_default(),
                "volume": r.get::<Option<String>, _>("volume_litre").unwrap_or_default(),
                "nodeTag": r.get::<String, _>("node_tag"),
                "traName": r.get::<String, _>("tra_name"),
                "synced_at": r.get::<String, _>("synced_at")
            }));
        }
        Ok(out)
    }

    pub async fn get_financial_summary(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> AppResult<serde_json::Value> {
        let s = start_date.unwrap_or("2026-01-01");
        let e = end_date.unwrap_or("2026-12-31");

        // 1. Total Allocations in window
        let alloc_row = sqlx::query(
            "SELECT COALESCE(SUM(amount_fen), 0) as total_fen, COUNT(*) as count 
             FROM local_allocations 
             WHERE substr(ope_time, 1, 10) >= ? AND substr(ope_time, 1, 10) <= ?"
        )
        .bind(s)
        .bind(e)
        .fetch_one(&self.pool)
        .await?;
        let total_alloc_fen: i64 = alloc_row.get("total_fen");
        let alloc_count: i64 = alloc_row.get("count");

        // 2. Total Quancun in window
        let quancun_row = sqlx::query(
            "SELECT COALESCE(SUM(amount_fen), 0) as total_fen, COUNT(*) as count 
             FROM local_quancun_records 
             WHERE substr(ope_time, 1, 10) >= ? AND substr(ope_time, 1, 10) <= ?"
        )
        .bind(s)
        .bind(e)
        .fetch_one(&self.pool)
        .await?;
        let total_quancun_fen: i64 = quancun_row.get("total_fen");
        let quancun_count: i64 = quancun_row.get("count");

        // 3. Total Consumption in window
        let tx_row = sqlx::query(
            "SELECT COALESCE(SUM(amount_fen), 0) as total_fen, COUNT(*) as count 
             FROM local_transactions 
             WHERE substr(ope_time, 1, 10) >= ? AND substr(ope_time, 1, 10) <= ? AND tra_name != '圈存'"
        )
        .bind(s)
        .bind(e)
        .fetch_one(&self.pool)
        .await?;
        let total_consumption_fen: i64 = tx_row.get("total_fen");
        let consumption_count: i64 = tx_row.get("count");

        // 4. Invoices in window
        let inv_row = sqlx::query(
            "SELECT COALESCE(SUM(amount_fen), 0) as total_fen, COUNT(*) as count 
             FROM local_invoices 
             WHERE substr(add_date, 1, 10) >= ? AND substr(add_date, 1, 10) <= ?"
        )
        .bind(s)
        .bind(e)
        .fetch_one(&self.pool)
        .await?;
        let total_inv_fen: i64 = inv_row.get("total_fen");
        let inv_count: i64 = inv_row.get("count");

        // Master pool balance from sync:allocations KV if available
        let cached_alloc = self.get_kv("sync:allocations").await?.unwrap_or_default();
        let master_pool_yuan = cached_alloc.get("master_pool_balance_yuan").and_then(|v| v.as_str()).unwrap_or("626.78");

        Ok(serde_json::json!({
            "window": { "start_date": s, "end_date": e },
            "master_pool_balance_yuan": master_pool_yuan,
            "allocations": {
                "count": alloc_count,
                "total_fen": total_alloc_fen,
                "total_yuan": crate::sinopec::models::format_fen_yuan(total_alloc_fen)
            },
            "quancun": {
                "count": quancun_count,
                "total_fen": total_quancun_fen,
                "total_yuan": crate::sinopec::models::format_fen_yuan(total_quancun_fen)
            },
            "consumption": {
                "count": consumption_count,
                "total_fen": total_consumption_fen,
                "total_yuan": crate::sinopec::models::format_fen_yuan(total_consumption_fen)
            },
            "invoices": {
                "count": inv_count,
                "total_fen": total_inv_fen,
                "total_yuan": crate::sinopec::models::format_fen_yuan(total_inv_fen)
            },
            "conservation": {
                "unquancun_diff_fen": total_alloc_fen - total_quancun_fen,
                "unquancun_diff_yuan": crate::sinopec::models::format_fen_yuan(total_alloc_fen - total_quancun_fen),
                "in_card_retention_fen": total_quancun_fen - total_consumption_fen,
                "in_card_retention_yuan": crate::sinopec::models::format_fen_yuan(total_quancun_fen - total_consumption_fen)
            }
        }))
    }
}
