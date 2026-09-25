use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::sinopec::capabilities::InvoiceCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountMode {
    Personal,
    #[default]
    Corporate,
}

impl AccountMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Corporate => "corporate",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_mode: AccountMode,
    pub member_account: String,
    pub company_name: Option<String>,
    pub customer_code: Option<String>,
    pub customer_type_code: String,
    pub customer_type_label: String,
    pub tax_code_masked: Option<String>,
    pub holder_name: Option<String>,
    pub bound_card_count: usize,
    pub invoice_type_label: String,
    pub verified_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub remote_id: String,
    pub masked_card_no: String,
    pub card_alias: Option<String>,
    pub card_type: String,
    pub is_master_card: bool,
    pub holder_name: Option<String>,
    pub company_name: Option<String>,
    pub tax_code_masked: Option<String>,
    pub province_code: String,
    pub province_name: String,
    pub customer_type: String,
    pub invoice_attribute: String,
    pub balance: String,
    pub balance_fen: i64,
    pub reserve_balance: String,
    pub reserve_balance_fen: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub remote_id: String,
    pub card_id: String,
    pub transaction_time: String,
    pub station: String,
    pub product: String,
    pub amount: String,
    pub amount_fen: i64,
    pub invoice_state: String,
    pub invoice_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceQuota {
    pub supported: bool,
    pub account_mode: AccountMode,
    pub card_id: Option<String>,
    pub available_amount: String,
    pub available_amount_fen: i64,
    pub currency: String,
    pub as_of: String,
    pub source_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePreviewRequest {
    pub start_date: String,
    pub end_date: String,
    pub card_id: Option<String>,
    pub invoice_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePreviewResponse {
    pub account_mode: AccountMode,
    pub start_date: String,
    pub end_date: String,
    pub date_windows_queried: Vec<(String, String)>,
    pub transaction_count: usize,
    pub total_amount: String,
    pub total_amount_fen: i64,
    pub invoice_type: String,
    pub card: Option<Card>,
    pub capabilities: InvoiceCapabilities,
    pub quota: Option<InvoiceQuota>,
    pub candidates: Vec<Transaction>,
    pub warnings: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub can_submit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceRequest {
    pub start_date: String,
    pub end_date: String,
    pub card_id: Option<String>,
    pub invoice_type: Option<String>,
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceRecord {
    pub id: String,
    pub invoice_no: Option<String>,
    pub invoice_date: String,
    pub invoice_type: String,
    pub buyer_name: String,
    pub amount: String,
    pub amount_fen: i64,
    pub status: String,
    pub file_format: String,
    pub file_status: String,
    pub download_path: Option<String>,
}

pub fn fen_to_decimal(fen: i64) -> Decimal {
    Decimal::new(fen, 2)
}

pub fn format_fen_yuan(fen: i64) -> String {
    format!("{:.2}", fen_to_decimal(fen))
}
