use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::AppResult;
use crate::research::Redactor;
use crate::sinopec::models::AccountMode;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredCredentials {
    #[serde(default)]
    pub account_mode: AccountMode,
    pub username: Option<String>,
    pub password: Option<String>,
    pub phone: Option<String>,
    /// Corporate login fields discovered from /corpgas/res/html/login/login_pc.jsp
    pub tax_code: Option<String>,
    pub tax_type: Option<String>,
    pub id_type: Option<String>,
    pub id_number: Option<String>,
    pub holder_name: Option<String>,
    pub master_card_no: Option<String>,
    pub province: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsStatusView {
    pub account_mode: AccountMode,
    pub username_configured: bool,
    pub password_configured: bool,
    pub phone_configured: bool,
    pub corporate_tax_configured: bool,
    pub corporate_id_configured: bool,
    pub corporate_holder_configured: bool,
    pub master_card_configured: bool,
    pub masked_phone: Option<String>,
    pub masked_tax_code: Option<String>,
    pub masked_master_card: Option<String>,
    pub province: Option<String>,
}

#[derive(Clone)]
pub struct CredentialProvider {
    file_path: PathBuf,
}

impl CredentialProvider {
    pub fn new(secrets_dir: PathBuf) -> Self {
        Self {
            file_path: secrets_dir.join("credentials.json"),
        }
    }

    pub fn load(&self) -> AppResult<Option<StoredCredentials>> {
        if !self.file_path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.file_path)?;
        let parsed: StoredCredentials = serde_json::from_str(&raw).unwrap_or_default();
        Ok(Some(parsed))
    }

    pub fn save(&self, incoming: StoredCredentials) -> AppResult<CredentialsStatusView> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existing = self.load()?.unwrap_or_default();
        let merged = StoredCredentials {
            account_mode: incoming.account_mode,
            username: Self::merge_opt(incoming.username, existing.username),
            password: Self::merge_opt(incoming.password, existing.password),
            phone: Self::merge_opt(incoming.phone, existing.phone),
            tax_code: Self::merge_opt(incoming.tax_code, existing.tax_code),
            tax_type: Self::merge_opt(incoming.tax_type, existing.tax_type)
                .or_else(|| Some("1".to_string())),
            id_type: Self::merge_opt(incoming.id_type, existing.id_type)
                .or_else(|| Some("01".to_string())),
            id_number: Self::merge_opt(incoming.id_number, existing.id_number),
            holder_name: Self::merge_opt(incoming.holder_name, existing.holder_name),
            master_card_no: Self::merge_opt(incoming.master_card_no, existing.master_card_no),
            province: Self::merge_opt(incoming.province, existing.province)
                .or_else(|| Some("12".to_string())),
        };
        let content = serde_json::to_string_pretty(&merged).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(&self.file_path, content)?;
        Ok(Self::to_status_view(Some(&merged)))
    }

    pub fn delete(&self) -> AppResult<()> {
        if self.file_path.exists() {
            std::fs::remove_file(&self.file_path)?;
        }
        Ok(())
    }

    pub fn status(&self) -> AppResult<CredentialsStatusView> {
        let creds = self.load()?;
        Ok(Self::to_status_view(creds.as_ref()))
    }

    pub fn has_usable_credentials(&self) -> bool {
        match self.load() {
            Ok(Some(c)) => match c.account_mode {
                AccountMode::Corporate => {
                    Self::non_empty(&c.tax_code)
                        && Self::non_empty(&c.id_number)
                        && Self::non_empty(&c.holder_name)
                        && Self::non_empty(&c.phone)
                }
                AccountMode::Personal => {
                    (Self::non_empty(&c.username) || Self::non_empty(&c.phone))
                        && Self::non_empty(&c.password)
                }
            },
            _ => false,
        }
    }

    fn to_status_view(creds: Option<&StoredCredentials>) -> CredentialsStatusView {
        match creds {
            Some(c) => CredentialsStatusView {
                account_mode: c.account_mode,
                username_configured: Self::non_empty(&c.username),
                password_configured: Self::non_empty(&c.password),
                phone_configured: Self::non_empty(&c.phone),
                corporate_tax_configured: Self::non_empty(&c.tax_code),
                corporate_id_configured: Self::non_empty(&c.id_number),
                corporate_holder_configured: Self::non_empty(&c.holder_name),
                master_card_configured: Self::non_empty(&c.master_card_no),
                masked_phone: c
                    .phone
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(Redactor::mask_phone),
                masked_tax_code: c
                    .tax_code
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(Redactor::mask_id_or_tax),
                masked_master_card: c
                    .master_card_no
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(Redactor::mask_card_number),
                province: c.province.clone(),
            },
            None => CredentialsStatusView {
                account_mode: AccountMode::Corporate,
                username_configured: false,
                password_configured: false,
                phone_configured: false,
                corporate_tax_configured: false,
                corporate_id_configured: false,
                corporate_holder_configured: false,
                master_card_configured: false,
                masked_phone: None,
                masked_tax_code: None,
                masked_master_card: None,
                province: Some("12".to_string()),
            },
        }
    }

    fn merge_opt(incoming: Option<String>, existing: Option<String>) -> Option<String> {
        match incoming {
            Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => existing,
        }
    }

    fn non_empty(opt: &Option<String>) -> bool {
        opt.as_ref().is_some_and(|s| !s.trim().is_empty())
    }
}
