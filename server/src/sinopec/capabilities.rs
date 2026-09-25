use serde::{Deserialize, Serialize};

use crate::sinopec::models::{AccountMode, Card};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceCapabilities {
    pub account_mode: AccountMode,
    pub card_type: String,
    pub invoice_attribute: String,
    pub supports_online_invoice: bool,
    pub supports_normal_invoice: bool,
    pub supports_special_invoice: bool,
    pub supports_invoice_reservation: bool,
    pub supports_invoice_download: bool,
    pub supports_quota_query: bool,
    pub invoice_title_locked: bool,
    pub requires_consumption_record: bool,
    pub requires_recharge_record: bool,
    pub max_query_window_days: u32,
    pub notes: Vec<String>,
}

impl InvoiceCapabilities {
    pub fn detect(account_mode: AccountMode, primary_card: Option<&Card>) -> Self {
        match (account_mode, primary_card) {
            (AccountMode::Corporate, Some(card)) => {
                let is_special = card.invoice_attribute == "special_vat"
                    || card.invoice_attribute.contains("增票")
                    || card.invoice_attribute.contains("专票");
                let is_master = card.is_master_card;
                let mut notes = vec![
                    "Corporate portal (/corpgas) locks invoice title & USCC tax number to the registered master card profile.".to_string(),
                    "Electronic invoice portal entry: /corpgas/webjsp/invoicev2/createInvoice.jsp & /corpgas/webjsp/invoicev2/queryInvoiceBill.jsp.".to_string(),
                ];
                if !is_master {
                    notes.push("Vice card detected (priCard != 1): online invoicing and pre-allocation must be initiated under the Master Card.".to_string());
                }

                Self {
                    account_mode: AccountMode::Corporate,
                    card_type: card.card_type.clone(),
                    invoice_attribute: card.invoice_attribute.clone(),
                    supports_online_invoice: is_master,
                    supports_normal_invoice: true,
                    supports_special_invoice: is_special,
                    supports_invoice_reservation: true,
                    supports_invoice_download: true,
                    supports_quota_query: true,
                    invoice_title_locked: true,
                    requires_consumption_record: true,
                    requires_recharge_record: false,
                    max_query_window_days: 31,
                    notes,
                }
            }
            (AccountMode::Corporate, None) => Self {
                account_mode: AccountMode::Corporate,
                card_type: "entity_card".to_string(),
                invoice_attribute: "normal_vat".to_string(),
                supports_online_invoice: true,
                supports_normal_invoice: true,
                supports_special_invoice: true,
                supports_invoice_reservation: true,
                supports_invoice_download: true,
                supports_quota_query: true,
                invoice_title_locked: true,
                requires_consumption_record: true,
                requires_recharge_record: false,
                max_query_window_days: 31,
                notes: vec![
                    "Corporate mode default profile before card binding check.".to_string(),
                ],
            },
            (AccountMode::Personal, card_opt) => Self {
                account_mode: AccountMode::Personal,
                card_type: card_opt
                    .map(|c| c.card_type.clone())
                    .unwrap_or_else(|| "entity_card".to_string()),
                invoice_attribute: "normal_vat".to_string(),
                supports_online_invoice: true,
                supports_normal_invoice: true,
                supports_special_invoice: false,
                supports_invoice_reservation: false,
                supports_invoice_download: true,
                supports_quota_query: false,
                invoice_title_locked: false,
                requires_consumption_record: true,
                requires_recharge_record: false,
                max_query_window_days: 31,
                notes: vec![
                    "Personal accounts support normal electronic VAT invoices without corporate quota pools.".to_string(),
                ],
            },
        }
    }
}
