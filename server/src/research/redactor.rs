use serde_json::Value;

pub struct Redactor;

impl Redactor {
    /// Masks a 19-digit Sinopec card number to `****1234`.
    pub fn mask_card_number(card_no: &str) -> String {
        let trimmed = card_no.trim();
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() >= 4 {
            let suffix: String = chars[chars.len() - 4..].iter().collect();
            format!("****{suffix}")
        } else {
            "****".to_string()
        }
    }

    /// Masks a phone number (`13812345678` -> `138****5678`).
    pub fn mask_phone(phone: &str) -> String {
        let chars: Vec<char> = phone.trim().chars().collect();
        if chars.len() == 11 {
            let prefix: String = chars[..3].iter().collect();
            let suffix: String = chars[7..].iter().collect();
            format!("{prefix}****{suffix}")
        } else if !chars.is_empty() {
            "***REDACTED_PHONE***".to_string()
        } else {
            String::new()
        }
    }

    /// Masks an 18-character ID card number or USCC tax code.
    pub fn mask_id_or_tax(value: &str) -> String {
        let chars: Vec<char> = value.trim().chars().collect();
        if chars.len() >= 8 {
            let prefix: String = chars[..3].iter().collect();
            let suffix: String = chars[chars.len() - 4..].iter().collect();
            format!("{prefix}***********{suffix}")
        } else if !chars.is_empty() {
            "***REDACTED***".to_string()
        } else {
            String::new()
        }
    }

    /// Redacts sensitive keys and patterns recursively in JSON values.
    pub fn redact_json(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, v) in map {
                    let lower = k.to_ascii_lowercase();
                    if Self::is_secret_key(&lower) {
                        out.insert(k.clone(), Value::String("<REDACTED>".to_string()));
                    } else if lower.contains("cardno")
                        || lower == "card_no"
                        || lower == "card_number"
                    {
                        if let Some(s) = v.as_str() {
                            out.insert(k.clone(), Value::String(Self::mask_card_number(s)));
                        } else {
                            out.insert(k.clone(), Self::redact_json(v));
                        }
                    } else if lower.contains("mobile") || lower.contains("phone") {
                        if let Some(s) = v.as_str() {
                            out.insert(k.clone(), Value::String(Self::mask_phone(s)));
                        } else {
                            out.insert(k.clone(), Self::redact_json(v));
                        }
                    } else if lower == "idno"
                        || lower == "tax"
                        || lower == "taxno"
                        || lower == "id_number"
                    {
                        if let Some(s) = v.as_str() {
                            out.insert(k.clone(), Value::String(Self::mask_id_or_tax(s)));
                        } else {
                            out.insert(k.clone(), Self::redact_json(v));
                        }
                    } else {
                        out.insert(k.clone(), Self::redact_json(v));
                    }
                }
                Value::Object(out)
            }
            Value::Array(arr) => Value::Array(arr.iter().map(Self::redact_json).collect()),
            Value::String(s) => Value::String(Self::redact_text(s)),
            other => other.clone(),
        }
    }

    /// Redacts digit runs (19-digit fuel cards, 18-digit IDs, 11-digit phones) and cookie headers in text.
    pub fn redact_text(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == 'X' || chars[i] == 'x')
                {
                    i += 1;
                }
                let run_len = i - start;
                let slice: String = chars[start..i].iter().collect();
                if run_len == 19 && slice.starts_with("100011") {
                    result.push_str(&Self::mask_card_number(&slice));
                } else if run_len == 18 {
                    result.push_str(&Self::mask_id_or_tax(&slice));
                } else if run_len == 11 && slice.starts_with('1') {
                    result.push_str(&Self::mask_phone(&slice));
                } else {
                    result.push_str(&slice);
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        for header in [
            "JSESSIONID=",
            "MYSERVERID_corpgas=",
            "Authorization:",
            "Bearer ",
        ] {
            if result.contains(header) {
                result = result.replace(header, &format!("{header}<REDACTED>;"));
            }
        }
        result
    }

    fn is_secret_key(lower_key: &str) -> bool {
        matches!(
            lower_key,
            "cookie"
                | "set-cookie"
                | "authorization"
                | "token"
                | "password"
                | "oldumm"
                | "newumm"
                | "memberumm"
                | "smsyzm"
                | "random_code_sms"
                | "sms_code"
                | "lastmsg"
        )
    }
}
