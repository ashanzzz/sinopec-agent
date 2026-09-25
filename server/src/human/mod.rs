use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::storage::SqliteStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HumanActionReason {
    Login,
    SmsCode,
    Captcha,
    Mfa,
    QrConfirm,
    DeviceConfirm,
    AccountSelection,
    Agreement,
    UnknownPage,
    BusinessConfirmation,
    LiveInvoiceConfirm,
}

impl HumanActionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Login => "LOGIN",
            Self::SmsCode => "SMS_CODE",
            Self::Captcha => "CAPTCHA",
            Self::Mfa => "MFA",
            Self::QrConfirm => "QR_CONFIRM",
            Self::DeviceConfirm => "DEVICE_CONFIRM",
            Self::AccountSelection => "ACCOUNT_SELECTION",
            Self::Agreement => "AGREEMENT",
            Self::UnknownPage => "UNKNOWN_PAGE",
            Self::BusinessConfirmation => "BUSINESS_CONFIRMATION",
            Self::LiveInvoiceConfirm => "LIVE_INVOICE_CONFIRM",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "LOGIN" => Self::Login,
            "SMS_CODE" => Self::SmsCode,
            "CAPTCHA" => Self::Captcha,
            "MFA" => Self::Mfa,
            "QR_CONFIRM" => Self::QrConfirm,
            "DEVICE_CONFIRM" => Self::DeviceConfirm,
            "ACCOUNT_SELECTION" => Self::AccountSelection,
            "AGREEMENT" => Self::Agreement,
            "BUSINESS_CONFIRMATION" => Self::BusinessConfirmation,
            "LIVE_INVOICE_CONFIRM" => Self::LiveInvoiceConfirm,
            _ => Self::UnknownPage,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HumanActionStatus {
    Waiting,
    Completed,
    Expired,
    Cancelled,
}

impl HumanActionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "WAITING",
            Self::Completed => "COMPLETED",
            Self::Expired => "EXPIRED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "COMPLETED" => Self::Completed,
            "EXPIRED" => Self::Expired,
            "CANCELLED" => Self::Cancelled,
            _ => Self::Waiting,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanActionRecord {
    pub id: String,
    pub token: String,
    pub reason: HumanActionReason,
    pub status: HumanActionStatus,
    pub message: String,
    pub human_action_url: String,
    pub viewer_url: Option<String>,
    pub operation_id: Option<String>,
    pub context: serde_json::Value,
    pub created_at: String,
    pub expires_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone)]
pub struct HumanActionManager {
    store: SqliteStore,
    public_base_url: String,
    default_viewer_url: String,
    /// Ephemeral in-memory SMS code store: never persisted to SQLite, logs, or disk.
    ephemeral_sms_codes: Arc<RwLock<HashMap<String, String>>>,
}

impl HumanActionManager {
    pub fn new(store: SqliteStore, public_base_url: String, default_viewer_url: String) -> Self {
        Self {
            store,
            public_base_url,
            default_viewer_url,
            ephemeral_sms_codes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_action(
        &self,
        reason: HumanActionReason,
        message: impl Into<String>,
        operation_id: Option<String>,
        viewer_url: Option<String>,
        context: serde_json::Value,
    ) -> AppResult<HumanActionRecord> {
        let id = format!("ha_{}", Uuid::new_v4().simple());
        let token = Uuid::new_v4().simple().to_string();
        let now = Utc::now();
        let expires = now + Duration::minutes(15);
        let viewer = viewer_url.or_else(|| Some(self.default_viewer_url.clone()));
        let msg = message.into();
        let ctx_str = serde_json::to_string(&context).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO human_actions (
                id, token, reason, status, message, operation_id, viewer_url,
                context_json, created_at, expires_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&token)
        .bind(reason.as_str())
        .bind(HumanActionStatus::Waiting.as_str())
        .bind(&msg)
        .bind(&operation_id)
        .bind(&viewer)
        .bind(&ctx_str)
        .bind(now.to_rfc3339())
        .bind(expires.to_rfc3339())
        .execute(self.store.pool())
        .await?;

        Ok(self.build_record(
            id,
            token,
            reason,
            HumanActionStatus::Waiting,
            msg,
            viewer,
            operation_id,
            context,
            now.to_rfc3339(),
            expires.to_rfc3339(),
            None,
        ))
    }

    pub async fn get_by_id_or_token(&self, id_or_token: &str) -> AppResult<HumanActionRecord> {
        let row = sqlx::query("SELECT * FROM human_actions WHERE id = ? OR token = ? LIMIT 1")
            .bind(id_or_token)
            .bind(id_or_token)
            .fetch_optional(self.store.pool())
            .await?
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidRequest,
                    format!("Human action not found: {id_or_token}"),
                )
            })?;

        Ok(self.row_to_record(&row))
    }

    pub async fn list_actions(&self) -> AppResult<Vec<HumanActionRecord>> {
        let rows = sqlx::query("SELECT * FROM human_actions ORDER BY created_at DESC LIMIT 50")
            .fetch_all(self.store.pool())
            .await?;
        Ok(rows.iter().map(|r| self.row_to_record(r)).collect())
    }

    pub async fn submit_ephemeral_sms_code(
        &self,
        id_or_token: &str,
        sms_code: String,
    ) -> AppResult<()> {
        let action = self.get_by_id_or_token(id_or_token).await?;
        let trimmed = sms_code.trim().to_string();
        if !trimmed.is_empty() {
            let mut guard = self.ephemeral_sms_codes.write().await;
            guard.insert(action.id, trimmed);
        }
        Ok(())
    }

    pub async fn take_ephemeral_sms_code(&self, action_id: &str) -> Option<String> {
        let mut guard = self.ephemeral_sms_codes.write().await;
        guard.remove(action_id)
    }

    pub async fn complete_action(&self, id_or_token: &str) -> AppResult<HumanActionRecord> {
        let record = self.get_by_id_or_token(id_or_token).await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE human_actions SET status = 'COMPLETED', completed_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&record.id)
            .execute(self.store.pool())
            .await?;
        // Always purge any ephemeral SMS code once action completes
        let _ = self.take_ephemeral_sms_code(&record.id).await;
        self.get_by_id_or_token(&record.id).await
    }

    pub async fn cancel_action(&self, id_or_token: &str) -> AppResult<HumanActionRecord> {
        let record = self.get_by_id_or_token(id_or_token).await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE human_actions SET status = 'CANCELLED', completed_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&record.id)
            .execute(self.store.pool())
            .await?;
        let _ = self.take_ephemeral_sms_code(&record.id).await;
        self.get_by_id_or_token(&record.id).await
    }

    #[allow(clippy::too_many_arguments)]
    fn build_record(
        &self,
        id: String,
        token: String,
        reason: HumanActionReason,
        status: HumanActionStatus,
        message: String,
        viewer_url: Option<String>,
        operation_id: Option<String>,
        context: serde_json::Value,
        created_at: String,
        expires_at: String,
        completed_at: Option<String>,
    ) -> HumanActionRecord {
        let human_action_url = format!(
            "{}/human/{}",
            self.public_base_url.trim_end_matches('/'),
            token
        );
        HumanActionRecord {
            id,
            token,
            reason,
            status,
            message,
            human_action_url,
            viewer_url,
            operation_id,
            context,
            created_at,
            expires_at,
            completed_at,
        }
    }

    fn row_to_record(&self, row: &sqlx::sqlite::SqliteRow) -> HumanActionRecord {
        let id: String = row.get("id");
        let token: String = row.get("token");
        let reason_str: String = row.get("reason");
        let status_str: String = row.get("status");
        let message: String = row.get("message");
        let viewer_url: Option<String> = row.get("viewer_url");
        let operation_id: Option<String> = row.get("operation_id");
        let ctx_str: String = row.get("context_json");
        let created_at: String = row.get("created_at");
        let expires_at: String = row.get("expires_at");
        let completed_at: Option<String> = row.get("completed_at");

        let mut status = HumanActionStatus::parse(&status_str);
        if status == HumanActionStatus::Waiting {
            if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&expires_at) {
                if exp.with_timezone(&Utc) < Utc::now() {
                    status = HumanActionStatus::Expired;
                }
            }
        }

        let context = serde_json::from_str(&ctx_str).unwrap_or_else(|_| serde_json::json!({}));
        self.build_record(
            id,
            token,
            HumanActionReason::parse(&reason_str),
            status,
            message,
            viewer_url,
            operation_id,
            context,
            created_at,
            expires_at,
            completed_at,
        )
    }
}
