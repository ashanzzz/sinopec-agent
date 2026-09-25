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
}
