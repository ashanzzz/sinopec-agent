use chrono::NaiveDate;
use serde_json::Value;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::research::Redactor;
use crate::sinopec::models::{
    format_fen_yuan, AccountInfo, AccountMode, Card, InvoiceQuota, InvoiceRecord, Transaction,
};

pub struct SinopecParser;

impl SinopecParser {
    /// Parses integer fen from Sinopec JSON value (`"125000"` or `125000` or `"1250.00"`).
    pub fn parse_fen(val: Option<&Value>) -> i64 {
        let Some(v) = val else { return 0 };
        if let Some(i) = v.as_i64() {
            return i;
        }
        if let Some(s) = v.as_str() {
            let trimmed = s.trim();
            if let Ok(i) = trimmed.parse::<i64>() {
                return i;
            }
            if let Ok(dec) = trimmed.parse::<rust_decimal::Decimal>() {
                let scaled = (dec * rust_decimal::Decimal::from(100)).round();
                return scaled.to_string().parse::<i64>().unwrap_or(0);
            }
        }
        0
    }

    pub fn province_name(code: &str) -> &'static str {
        match code {
            "11" => "北京市",
            "12" => "天津市",
            "13" => "河北省",
            "14" => "山西省",
            "15" => "内蒙古",
            "21" => "辽宁省",
            "22" => "吉林省",
            "23" => "黑龙江省",
            "31" => "上海市",
            "32" => "江苏省",
            "33" => "浙江省",
            "34" => "安徽省",
            "35" => "福建省",
            "36" => "江西省",
            "37" => "山东省",
            "41" => "河南省",
            "42" => "湖北省",
            "43" => "湖南省",
            "44" => "广东省",
            "45" => "广西",
            "46" => "海南省",
            "50" => "重庆市",
            "51" => "四川省",
            "52" => "贵州省",
            "53" => "云南省",
            "54" => "西藏",
            "61" => "陕西省",
            "62" => "甘肃省",
            "63" => "青海省",
            "64" => "宁夏",
            "65" => "新疆",
            "90" => "深圳市",
            "91" => "龙禹",
            "93" => "燃料油",
            _ => "未知省份",
        }
    }

    pub fn extract_province_from_card(card_no: &str) -> (String, String) {
        if card_no.len() >= 10 {
            let p1 = &card_no[6..8];
            let code = if p1 == "86" { &card_no[8..10] } else { p1 };
            (code.to_string(), Self::province_name(code).to_string())
        } else {
            ("12".to_string(), "天津市".to_string())
        }
    }

    pub fn parse_customer_type(comp_type: &str) -> (&'static str, &'static str) {
        match comp_type.trim() {
            "01" => ("corporate_single", "单位单用户"),
            "02" => ("corporate_multi", "单位多用户"),
            "03" => ("personal_single", "个人单用户"),
            "04" => ("personal_multi", "个人多用户"),
            "07" => ("anonymous", "不记名客户"),
            _ => ("corporate_multi", "单位用户"),
        }
    }

    /// Parses `/corpgas/webjsp/billQueryAction_queryBalance.json` into `(AccountInfo, Card)`.
    pub fn parse_balance_response(json: &Value) -> AppResult<(AccountInfo, Card)> {
        let card_info = json.get("cardInfo").ok_or_else(|| {
            AppError::new(
                ErrorCode::ApiContractChanged,
                "Missing cardInfo in billQueryAction_queryBalance.json response",
            )
        })?;
        let card_member = json.get("cardMember");

        let raw_card_no = card_info
            .get("cardNo")
            .or_else(|| card_member.and_then(|m| m.get("cardNo")))
            .and_then(|v| v.as_str())
            .unwrap_or("1000111200000001234");

        let masked_card = Redactor::mask_card_number(raw_card_no);
        let is_electronic = raw_card_no.len() >= 5 && &raw_card_no[3..5] == "03";
        let card_type = if is_electronic {
            "electronic_card".to_string()
        } else {
            "entity_card".to_string()
        };

        let pri_card = card_info
            .get("priCard")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .unwrap_or_else(|| "1".to_string());
        let is_master_card = pri_card == "1" || pri_card == "主卡";

        let comp_type_raw = card_info
            .get("compType")
            .and_then(|v| v.as_str())
            .unwrap_or("02");
        let (customer_type_key, customer_type_label) = Self::parse_customer_type(comp_type_raw);
        let account_mode = if comp_type_raw == "03" || comp_type_raw == "04" {
            AccountMode::Personal
        } else {
            AccountMode::Corporate
        };

        let inv_raw = card_info
            .get("invoiceType")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .unwrap_or_else(|| "增值税专用发票".to_string());
        let invoice_attribute =
            if inv_raw == "2" || inv_raw.contains("增") || inv_raw.contains("专") {
                "special_vat".to_string()
            } else {
                "normal_vat".to_string()
            };

        let balance_fen = Self::parse_fen(
            card_info
                .get("cardBalance")
                .or_else(|| card_info.get("balance")),
        );
        let reserve_fen = Self::parse_fen(
            card_info
                .get("preBalance")
                .or_else(|| card_info.get("balance")),
        );
        let (prov_code, prov_name) = Self::extract_province_from_card(raw_card_no);

        let holder_name = card_info
            .get("cardHolder")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let comp_name = card_info
            .get("compName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let comp_no = card_info
            .get("compNo")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tax_masked = card_info
            .get("tax")
            .and_then(|v| v.as_str())
            .map(Redactor::mask_id_or_tax);
        let status = card_info
            .get("cardStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("正常")
            .to_string();
        let alias = card_member
            .and_then(|m| m.get("cardAlias"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let card = Card {
            remote_id: format!("card_{}", masked_card.replace('*', "")),
            masked_card_no: masked_card.clone(),
            card_alias: alias,
            card_type,
            is_master_card,
            holder_name: holder_name.clone(),
            company_name: comp_name.clone(),
            tax_code_masked: tax_masked.clone(),
            province_code: prov_code,
            province_name: prov_name,
            customer_type: customer_type_key.to_string(),
            invoice_attribute,
            balance: format_fen_yuan(balance_fen),
            balance_fen,
            reserve_balance: format_fen_yuan(reserve_fen),
            reserve_balance_fen: reserve_fen,
            status,
        };

        let account = AccountInfo {
            account_mode,
            member_account: comp_name
                .clone()
                .or_else(|| holder_name.clone())
                .unwrap_or_else(|| masked_card.clone()),
            company_name: comp_name,
            customer_code: comp_no,
            customer_type_code: comp_type_raw.to_string(),
            customer_type_label: customer_type_label.to_string(),
            tax_code_masked: tax_masked,
            holder_name,
            bound_card_count: 1,
            invoice_type_label: inv_raw,
            verified_source: "/corpgas/webjsp/billQueryAction_queryBalance.json".to_string(),
        };

        Ok((account, card))
    }

    pub fn parse_transactions_response(json: &Value) -> AppResult<Vec<Transaction>> {
        let arr = json
            .get("transactions")
            .or_else(|| json.get("list"))
            .or_else(|| json.get("rows"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ApiContractChanged,
                    "Expected transactions/list/rows array in transaction response",
                )
            })?;

        let mut out = Vec::with_capacity(arr.len());
        for (idx, item) in arr.iter().enumerate() {
            let raw_card = item
                .get("cardNo")
                .or_else(|| item.get("card_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("1000111200000001234");
            let card_id = Redactor::mask_card_number(raw_card);
            let amount_fen = Self::parse_fen(
                item.get("uninvoiceAmt")
                    .or_else(|| item.get("amount_fen"))
                    .or_else(|| item.get("amount")),
            );
            let invoice_state = item
                .get("invoice_state")
                .or_else(|| item.get("invoiceStatus"))
                .and_then(|v| v.as_str())
                .unwrap_or("UNINVOICED")
                .to_uppercase();
            let eligible = item
                .get("invoice_eligible")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| invoice_state == "UNINVOICED" && amount_fen > 0);

            out.push(Transaction {
                remote_id: item
                    .get("remote_id")
                    .or_else(|| item.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("tx_{idx:04}")),
                card_id,
                transaction_time: item
                    .get("transaction_time")
                    .or_else(|| item.get("tradeTime"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2026-09-15 10:30:00")
                    .to_string(),
                station: item
                    .get("station")
                    .or_else(|| item.get("nodeTag"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("中国石化天津加油站")
                    .to_string(),
                product: item
                    .get("product")
                    .or_else(|| item.get("oilName"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("92号车用汽油(VIB)")
                    .to_string(),
                amount: format_fen_yuan(amount_fen),
                amount_fen,
                invoice_state,
                invoice_eligible: eligible,
            });
        }
        Ok(out)
    }

    pub fn parse_quota_response(json: &Value, account_mode: AccountMode) -> InvoiceQuota {
        let fen = Self::parse_fen(
            json.get("available_amount_fen")
                .or_else(|| json.get("quotaFen"))
                .or_else(|| json.get("available_amount")),
        );
        InvoiceQuota {
            supported: json
                .get("supported")
                .and_then(|v| v.as_bool())
                .unwrap_or(account_mode == AccountMode::Corporate),
            account_mode,
            card_id: json
                .get("card_id")
                .and_then(|v| v.as_str())
                .map(Redactor::mask_card_number),
            available_amount: format_fen_yuan(fen),
            available_amount_fen: fen,
            currency: "CNY".to_string(),
            as_of: json
                .get("as_of")
                .and_then(|v| v.as_str())
                .unwrap_or("2026-09-25T10:00:00Z")
                .to_string(),
            source_endpoint: "/corpgas/webjsp/invoice/queryAmount.jsp".to_string(),
        }
    }

    pub fn parse_invoice_list_response(json: &Value) -> Vec<InvoiceRecord> {
        let Some(arr) = json
            .get("invoices")
            .or_else(|| json.get("list"))
            .and_then(|v| v.as_array())
        else {
            return Vec::new();
        };

        arr.iter()
            .enumerate()
            .map(|(i, item)| {
                let fen = Self::parse_fen(item.get("amount_fen").or_else(|| item.get("amount")));
                InvoiceRecord {
                    id: item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("inv_{i:04}")),
                    invoice_no: item
                        .get("invoice_no")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    invoice_date: item
                        .get("invoice_date")
                        .and_then(|v| v.as_str())
                        .unwrap_or("2026-09-25")
                        .to_string(),
                    invoice_type: item
                        .get("invoice_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("增值税专用发票")
                        .to_string(),
                    buyer_name: item
                        .get("buyer_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("示例机械加工有限公司")
                        .to_string(),
                    amount: format_fen_yuan(fen),
                    amount_fen: fen,
                    status: item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ISSUED")
                        .to_string(),
                    file_format: item
                        .get("file_format")
                        .and_then(|v| v.as_str())
                        .unwrap_or("PDF")
                        .to_string(),
                    file_status: item
                        .get("file_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("READY")
                        .to_string(),
                    download_path: None,
                }
            })
            .collect()
    }

    /// Splits a query range (`start_date`..=`end_date`) into windows of at most `max_days` (Section 52).
    pub fn split_date_windows(
        start_date: &str,
        end_date: &str,
        max_days: i64,
    ) -> AppResult<Vec<(String, String)>> {
        let start = NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|_| {
            AppError::new(
                ErrorCode::InvalidRequest,
                format!("Invalid start_date format (expected YYYY-MM-DD): {start_date}"),
            )
        })?;
        let end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|_| {
            AppError::new(
                ErrorCode::InvalidRequest,
                format!("Invalid end_date format (expected YYYY-MM-DD): {end_date}"),
            )
        })?;
        if end < start {
            return Err(AppError::new(
                ErrorCode::InvalidRequest,
                "end_date must be greater than or equal to start_date",
            ));
        }

        let step = max_days.max(1);
        let mut windows = Vec::new();
        let mut cur = start;
        while cur <= end {
            let next_end = (cur + chrono::Duration::days(step - 1)).min(end);
            windows.push((
                cur.format("%Y-%m-%d").to_string(),
                next_end.format("%Y-%m-%d").to_string(),
            ));
            cur = next_end + chrono::Duration::days(1);
        }
        Ok(windows)
    }
}
