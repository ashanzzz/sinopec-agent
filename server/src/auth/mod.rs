pub mod captcha;
pub mod credentials;
pub mod verifier;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::credentials::{CredentialProvider, CredentialsStatusView};
use crate::auth::verifier::{AuthVerifier, VerificationOutcome};
use crate::error::AppResult;
use crate::human::HumanActionRecord;
use crate::sinopec::models::AccountMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthState {
    Unknown,
    Checking,
    CredentialsRequired,
    LoggedOut,
    AutoLogin,
    LoginInProgress,
    SmsRequired,
    CaptchaRequired,
    MfaRequired,
    HumanActionRequired,
    AuthUnverified,
    LoggedIn,
    SessionExpired,
    RateLimited,
    AccountLocked,
    Error,
}

impl AuthState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Checking => "CHECKING",
            Self::CredentialsRequired => "CREDENTIALS_REQUIRED",
            Self::LoggedOut => "LOGGED_OUT",
            Self::AutoLogin => "AUTO_LOGIN",
            Self::LoginInProgress => "LOGIN_IN_PROGRESS",
            Self::SmsRequired => "SMS_REQUIRED",
            Self::CaptchaRequired => "CAPTCHA_REQUIRED",
            Self::MfaRequired => "MFA_REQUIRED",
            Self::HumanActionRequired => "HUMAN_ACTION_REQUIRED",
            Self::AuthUnverified => "AUTH_UNVERIFIED",
            Self::LoggedIn => "LOGGED_IN",
            Self::SessionExpired => "SESSION_EXPIRED",
            Self::RateLimited => "RATE_LIMITED",
            Self::AccountLocked => "ACCOUNT_LOCKED",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub state: AuthState,
    pub account_mode: AccountMode,
    pub verified: bool,
    pub member_account: Option<String>,
    pub company_name: Option<String>,
    pub card_count: usize,
    pub auth_attempts: u32,
    pub max_auth_attempts: u32,
    pub last_checked_at: Option<String>,
    pub reason: String,
    pub credentials: CredentialsStatusView,
    pub active_human_action: Option<HumanActionRecord>,
}

struct InnerAuthRuntime {
    state: AuthState,
    account_mode: AccountMode,
    verified: bool,
    member_account: Option<String>,
    company_name: Option<String>,
    card_count: usize,
    auth_attempts: u32,
    last_checked_at: Option<String>,
    reason: String,
    active_human_action: Option<HumanActionRecord>,
}

#[derive(Clone)]
pub struct AuthManager {
    credentials: CredentialProvider,
    verifier: AuthVerifier,
    auth_dir: PathBuf,
    max_auth_attempts: u32,
    runtime: Arc<RwLock<InnerAuthRuntime>>,
}

impl AuthManager {
    pub fn new(
        credentials: CredentialProvider,
        verifier: AuthVerifier,
        auth_dir: PathBuf,
        max_auth_attempts: u32,
    ) -> Self {
        let initial_creds = credentials.status().ok();
        let initial_mode = initial_creds
            .as_ref()
            .map(|c| c.account_mode)
            .unwrap_or(AccountMode::Corporate);
        let initial_state = if credentials.has_usable_credentials() {
            AuthState::LoggedOut
        } else {
            AuthState::CredentialsRequired
        };

        Self {
            credentials,
            verifier,
            auth_dir,
            max_auth_attempts,
            runtime: Arc::new(RwLock::new(InnerAuthRuntime {
                state: initial_state,
                account_mode: initial_mode,
                verified: false,
                member_account: None,
                company_name: None,
                card_count: 0,
                auth_attempts: 0,
                last_checked_at: None,
                reason: "Initial state before remote verification".to_string(),
                active_human_action: None,
            })),
        }
    }

    pub fn credentials(&self) -> &CredentialProvider {
        &self.credentials
    }

    pub fn verifier(&self) -> &AuthVerifier {
        &self.verifier
    }

    pub fn session_cookie_path(&self) -> PathBuf {
        self.auth_dir.join("sinopec_cookies.json")
    }

    pub async fn status(&self) -> AppResult<AuthStatusResponse> {
        let creds = self.credentials.status()?;
        let guard = self.runtime.read().await;
        Ok(AuthStatusResponse {
            state: guard.state,
            account_mode: creds.account_mode,
            verified: guard.verified,
            member_account: guard.member_account.clone(),
            company_name: guard.company_name.clone(),
            card_count: guard.card_count,
            auth_attempts: guard.auth_attempts,
            max_auth_attempts: self.max_auth_attempts,
            last_checked_at: guard.last_checked_at.clone(),
            reason: guard.reason.clone(),
            credentials: creds,
            active_human_action: guard.active_human_action.clone(),
        })
    }

    pub async fn apply_verification(&self, outcome: VerificationOutcome) {
        let mut guard = self.runtime.write().await;
        guard.state = outcome.state;
        guard.account_mode = outcome.account_mode;
        guard.verified = outcome.verified;
        guard.member_account = outcome.member_account;
        guard.company_name = outcome.company_name;
        guard.card_count = outcome.card_count;
        guard.last_checked_at = Some(Utc::now().to_rfc3339());
        guard.reason = outcome.reason;
        if outcome.verified {
            guard.auth_attempts = 0;
            guard.active_human_action = None;
        }
    }

    pub async fn set_human_action_required(
        &self,
        state: AuthState,
        reason: impl Into<String>,
        human_action: HumanActionRecord,
    ) {
        let mut guard = self.runtime.write().await;
        guard.state = state;
        guard.verified = false;
        guard.last_checked_at = Some(Utc::now().to_rfc3339());
        guard.reason = reason.into();
        guard.active_human_action = Some(human_action);
    }

    pub async fn record_attempt(&self) -> u32 {
        let mut guard = self.runtime.write().await;
        guard.auth_attempts += 1;
        guard.auth_attempts
    }
}
