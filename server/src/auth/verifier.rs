use serde::{Deserialize, Serialize};

use crate::auth::AuthState;
use crate::error::AppResult;
use crate::sinopec::http::SinopecHttpTransport;
use crate::sinopec::models::AccountMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutcome {
    pub verified: bool,
    pub state: AuthState,
    pub account_mode: AccountMode,
    pub member_account: Option<String>,
    pub card_count: usize,
    pub master_card_masked: Option<String>,
    pub company_name: Option<String>,
    pub reason: String,
}

#[derive(Clone)]
pub struct AuthVerifier {
    http: SinopecHttpTransport,
}

impl AuthVerifier {
    pub fn new(http: SinopecHttpTransport) -> Self {
        Self { http }
    }

    /// Calls Sinopec's real read-only session & balance endpoints to verify live authentication.
    pub async fn verify_remote_session(
        &self,
        preferred_mode: AccountMode,
    ) -> AppResult<VerificationOutcome> {
        if !self.http.has_session_cookies().await {
            return Ok(VerificationOutcome {
                verified: false,
                state: AuthState::LoggedOut,
                account_mode: preferred_mode,
                member_account: None,
                card_count: 0,
                master_card_masked: None,
                company_name: None,
                reason: "No Sinopec session cookies stored in local session jar".to_string(),
            });
        }

        match self.http.probe_corporate_balance().await {
            Ok(Some(balance_info)) => {
                return Ok(VerificationOutcome {
                    verified: true,
                    state: AuthState::LoggedIn,
                    account_mode: AccountMode::Corporate,
                    member_account: balance_info
                        .company_name
                        .clone()
                        .or(balance_info.holder_name.clone()),
                    card_count: balance_info.card_count.max(1),
                    master_card_masked: Some(balance_info.masked_card_no),
                    company_name: balance_info.company_name,
                    reason: "Verified via /corpgas/webjsp/billQueryAction_queryBalance.json"
                        .to_string(),
                });
            }
            Ok(None) => {}
            Err(_) => {}
        }

        if let Ok(login_check) = self.http.check_login_or_out(preferred_mode).await {
            if !login_check.member_account.trim().is_empty() {
                return Ok(VerificationOutcome {
                    verified: true,
                    state: AuthState::LoggedIn,
                    account_mode: preferred_mode,
                    member_account: Some(login_check.member_account),
                    card_count: login_check.card_num as usize,
                    master_card_masked: None,
                    company_name: None,
                    reason: "Verified via memberLoginAction_logInOrOut.json".to_string(),
                });
            }
        }

        Ok(VerificationOutcome {
            verified: false,
            state: AuthState::SessionExpired,
            account_mode: preferred_mode,
            member_account: None,
            card_count: 0,
            master_card_masked: None,
            company_name: None,
            reason: "Upstream returned HTTP 390 or empty memberAccount (session expired)"
                .to_string(),
        })
    }
}
