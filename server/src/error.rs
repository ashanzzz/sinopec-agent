use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::human::HumanActionRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    CredentialsRequired,
    LoginRequired,
    SmsCodeRequired,
    CaptchaRequired,
    MfaRequired,
    HumanActionRequired,
    AuthUnverified,
    SessionExpired,
    RateLimited,
    AccountLocked,
    AccountModeUnsupported,
    InvoiceCapabilityUnknown,
    NoInvoiceableRecords,
    InvoiceNotSupported,
    InvoiceAlreadySubmitted,
    RemoteResultUnknown,
    ApiContractChanged,
    SteelUnavailable,
    SinopecUnavailable,
    InvalidRequest,
    InternalError,
}

impl ErrorCode {
    pub fn status_code(self) -> StatusCode {
        match self {
            Self::CredentialsRequired
            | Self::LoginRequired
            | Self::SmsCodeRequired
            | Self::CaptchaRequired
            | Self::MfaRequired
            | Self::HumanActionRequired
            | Self::AuthUnverified
            | Self::SessionExpired => StatusCode::UNAUTHORIZED,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::AccountLocked => StatusCode::FORBIDDEN,
            Self::InvalidRequest
            | Self::NoInvoiceableRecords
            | Self::InvoiceNotSupported
            | Self::AccountModeUnsupported => StatusCode::BAD_REQUEST,
            Self::InvoiceAlreadySubmitted => StatusCode::CONFLICT,
            Self::RemoteResultUnknown | Self::ApiContractChanged => StatusCode::BAD_GATEWAY,
            Self::SteelUnavailable | Self::SinopecUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::InvoiceCapabilityUnknown | Self::InternalError => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("[{code:?}] {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub human_action: Option<Box<HumanActionRecord>>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            human_action: None,
        }
    }

    pub fn with_human_action(
        code: ErrorCode,
        message: impl Into<String>,
        human_action: HumanActionRecord,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            human_action: Some(Box::new(human_action)),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::new(ErrorCode::InternalError, format!("Database error: {err}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::new(
                ErrorCode::SinopecUnavailable,
                format!("Upstream request timed out: {err}"),
            )
        } else {
            Self::new(
                ErrorCode::SinopecUnavailable,
                format!("Upstream HTTP transport error: {err}"),
            )
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::InternalError, format!("IO error: {err}"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiEnvelope<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_action: Option<Box<HumanActionRecord>>,
}

impl<T: Serialize> ApiEnvelope<T> {
    pub fn success(data: T) -> Json<Self> {
        Json(Self {
            ok: true,
            data: Some(data),
            error: None,
        })
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.code.status_code();
        let body = ApiEnvelope::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some(ApiErrorBody {
                code: self.code,
                message: self.message,
                human_action: self.human_action,
            }),
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
