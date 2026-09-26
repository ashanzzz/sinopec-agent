use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::browser::driver::{BrowserCookie, BrowserDriver, BrowserSessionInfo, BrowserSnapshot};
use crate::error::{AppError, AppResult, ErrorCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteelHealthStatus {
    pub reachable: bool,
    pub base_url: String,
    pub cdp_url: String,
    pub active_sessions: usize,
    pub active_session_id: Option<String>,
    pub viewer_url: String,
    pub browser_version: Option<String>,
}

#[derive(Clone)]
pub struct SteelBrowserDriver {
    base_url: String,
    cdp_url: String,
    client: reqwest::Client,
}

impl SteelBrowserDriver {
    pub fn new(base_url: String, cdp_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            cdp_url: cdp_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn check_health(&self) -> SteelHealthStatus {
        let sessions_url = format!("{}/v1/sessions", self.base_url);
        let cdp_http = self
            .cdp_url
            .replace("ws://", "http://")
            .replace("wss://", "https://");
        let version_url = format!("{cdp_http}/json/version");

        let sessions_res = self.client.get(&sessions_url).send().await;
        let version_res = self.client.get(&version_url).send().await;

        let mut active_sessions = 0;
        let mut active_session_id = None;
        let mut reachable = false;

        if let Ok(resp) = sessions_res {
            if resp.status().is_success() {
                reachable = true;
                if let Ok(val) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = val.get("sessions").and_then(|v| v.as_array()) {
                        active_sessions = arr.len();
                        active_session_id = arr
                            .first()
                            .and_then(|s| s.get("id"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }

        let browser_version = match version_res {
            Ok(r) if r.status().is_success() => {
                reachable = true;
                r.json::<serde_json::Value>().await.ok().and_then(|j| {
                    j.get("Browser")
                        .and_then(|b| b.as_str())
                        .map(|s| s.to_string())
                })
            }
            _ => None,
        };

        SteelHealthStatus {
            reachable,
            base_url: self.base_url.clone(),
            cdp_url: self.cdp_url.clone(),
            active_sessions,
            active_session_id,
            viewer_url: format!("{}/", self.base_url),
            browser_version,
        }
    }

    /// Extracts Sinopec cookies from the active Steel session context so `SinopecHttpTransport`
    /// can reuse browser-established sessions without keeping the browser open.
    pub async fn sync_sinopec_cookies(&self) -> AppResult<HashMap<String, String>> {
        let cookies = self.cookies().await?;
        let mut map = HashMap::new();
        for c in cookies {
            if c.domain.contains("sinopecsales.com") {
                map.insert(c.name, c.value);
            }
        }
        Ok(map)
    }

    async fn resolve_cdp_page_ws(&self) -> AppResult<String> {
        let cdp_http = self
            .cdp_url
            .replace("ws://", "http://")
            .replace("wss://", "https://");
        let list_url = format!("{cdp_http}/json/list");
        let resp = self.client.get(&list_url).send().await.map_err(|e| {
            AppError::new(
                ErrorCode::SteelUnavailable,
                format!("Steel CDP list failed: {e}"),
            )
        })?;
        let targets: Vec<serde_json::Value> = resp.json().await.map_err(|e| {
            AppError::new(
                ErrorCode::SteelUnavailable,
                format!("Steel CDP JSON error: {e}"),
            )
        })?;

        let target = targets
            .iter()
            .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
            .or_else(|| targets.first())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::SteelUnavailable,
                    "No CDP page target found in Steel",
                )
            })?;

        let raw_ws = target
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::new(ErrorCode::SteelUnavailable, "Missing webSocketDebuggerUrl")
            })?;

        let host_port = self
            .cdp_url
            .trim_start_matches("ws://")
            .trim_start_matches("wss://");
        if let Some(idx) = raw_ws.find("/devtools/") {
            Ok(format!("ws://{host_port}{}", &raw_ws[idx..]))
        } else {
            Ok(raw_ws.to_string())
        }
    }

    async fn run_cdp_command(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> AppResult<serde_json::Value> {
        let ws_url = self.resolve_cdp_page_ws().await?;
        let (mut ws_stream, _) = connect_async(&ws_url).await.map_err(|e| {
            AppError::new(
                ErrorCode::SteelUnavailable,
                format!("CDP connect failed: {e}"),
            )
        })?;

        let req = serde_json::json!({
            "id": 1,
            "method": method,
            "params": params,
        });
        ws_stream
            .send(Message::Text(req.to_string().into()))
            .await
            .map_err(|e| {
                AppError::new(ErrorCode::SteelUnavailable, format!("CDP send failed: {e}"))
            })?;

        while let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| AppError::new(ErrorCode::SteelUnavailable, e.to_string()))?;
            if let Message::Text(text) = msg {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if json.get("id").and_then(|v| v.as_i64()) == Some(1) {
                        let _ = ws_stream.close(None).await;
                        return Ok(json
                            .get("result")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null));
                    }
                }
            }
        }
        Err(AppError::new(
            ErrorCode::SteelUnavailable,
            "CDP stream closed before command response",
        ))
    }
}
impl SteelBrowserDriver {
    /// Fills corporate login credentials inside Steel Browser's `#log_ifame` (`login_pc.jsp`),
    /// extracts the live `img.random_code` pixels via HTML5 canvas, solves the arithmetic expression
    /// using `SinopecCaptchaSolver`, fills `#random_code_img`, checks `#hyzc`, and clicks `#srand`.
    pub async fn auto_fill_and_send_sms_in_browser(
        &self,
        creds: &crate::auth::credentials::StoredCredentials,
        click_send_sms: bool,
    ) -> AppResult<serde_json::Value> {
        let _ = self
            .evaluate("if (typeof showBg === 'function') { showBg('corp'); }")
            .await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let tax = creds.tax_code.clone().unwrap_or_default();
        let taxtype = creds.tax_type.clone().unwrap_or_else(|| "1".to_string());
        let idtype = creds.id_type.clone().unwrap_or_else(|| "01".to_string());
        let idno = creds.id_number.clone().unwrap_or_default();
        let name = creds.holder_name.clone().unwrap_or_default();
        let mobile = creds.phone.clone().unwrap_or_default();
        let _cardno = creds.master_card_no.clone().unwrap_or_default();

        // Step 1: Fill fields and extract canvas JPEG data URL from img.random_code
        let extract_js = format!(
            r#"(() => {{
                const iframe = document.getElementById('log_ifame');
                const doc = iframe ? (iframe.contentDocument || iframe.contentWindow.document) : document;
                if (!doc) return {{ ok: false, error: 'no login_pc.jsp document' }};
                const setVal = (id, val) => {{
                    const el = doc.getElementById(id);
                    if (el) {{
                        el.removeAttribute('disabled');
                        el.value = val;
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }}
                }};
                setVal('taxtype', {taxtype:?});
                setVal('tax', {tax:?});
                setVal('idtype', {idtype:?});
                setVal('idno', {idno:?});
                setVal('idno_hide', {idno:?});
                setVal('name', {name:?});
                setVal('name_hide', {name:?});
                setVal('member_phone', {mobile:?});
                setVal('member_phone_hide', {mobile:?});
                // Corporate multi-user login: cardno_div stays hidden and card_no empty
                const cdiv = doc.getElementById('cardno_div');
                if (cdiv) cdiv.classList.add('hide');
                setVal('card_no', '');
                const hyzc = doc.getElementById('hyzc');
                if (hyzc) {{
                    hyzc.checked = true;
                    hyzc.setAttribute('checked', 'checked');
                }}
                const img = doc.querySelector('img.random_code');
                let b64 = null;
                if (img && img.complete && img.naturalWidth > 0) {{
                    const canvas = doc.createElement('canvas');
                    canvas.width = img.naturalWidth;
                    canvas.height = img.naturalHeight;
                    const ctx = canvas.getContext('2d');
                    ctx.drawImage(img, 0, 0);
                    b64 = canvas.toDataURL('image/jpeg', 0.95).split(',')[1];
                }}
                return {{ ok: true, b64 }};
            }})()"#
        );

        let eval_val = self.evaluate(&extract_js).await?;
        let b64_opt = eval_val
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("b64"))
            .and_then(|s| s.as_str());

        let mut solved_expr = String::from("已自动填充");
        let mut solved_ans = String::new();

        if let Some(b64_str) = b64_opt {
            if let Ok(jpeg_bytes) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_str)
            {
                if let Ok(solved) =
                    crate::auth::captcha::SinopecCaptchaSolver::solve_jpeg(&jpeg_bytes)
                {
                    solved_expr = solved.expression;
                    solved_ans = solved.answer.to_string();
                }
            }
        }

        let step2_js = format!(
            r#"(() => {{
                const iframe = document.getElementById('log_ifame');
                const doc = iframe ? (iframe.contentDocument || iframe.contentWindow.document) : document;
                if (!doc) return {{ ok: false }};
                if ({solved_ans:?}.length > 0) {{
                    const el = doc.getElementById('random_code_img');
                    if (el) {{
                        el.value = {solved_ans:?};
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    }}
                }}
                const srand = doc.getElementById('srand');
                if ({click_send_sms} && srand && !srand.classList.contains('disabled')) {{
                    srand.click();
                }}
                return {{
                    ok: true,
                    srand_text: srand ? srand.innerText : null,
                    random_code_img: doc.getElementById('random_code_img')?.value
                }};
            }})()"#
        );
        let step2_res = self.evaluate(&step2_js).await?;

        Ok(serde_json::json!({
            "filled_in_steel": true,
            "captcha_expression": solved_expr,
            "captcha_answer": solved_ans,
            "browser_state": step2_res.get("result").and_then(|r| r.get("value")).cloned()
        }))
    }

    /// Fills the 6-digit SMS code into `#random_code_sms` and clicks `#login_sms` inside Steel Browser.
    pub async fn submit_sms_in_browser(&self, sms_code: &str) -> AppResult<serde_json::Value> {
        let code = sms_code.trim();
        let js = format!(
            r#"(() => {{
                const iframe = document.getElementById('log_ifame');
                const doc = iframe ? (iframe.contentDocument || iframe.contentWindow.document) : document;
                if (!doc) return {{ ok: false, error: 'no login_pc.jsp doc' }};
                const hyzc = doc.getElementById('hyzc');
                if (hyzc) {{
                    hyzc.checked = true;
                    hyzc.setAttribute('checked', 'checked');
                }}
                const smsInput = doc.getElementById('random_code_sms');
                if (smsInput) {{
                    smsInput.value = {code:?};
                    smsInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    smsInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }}
                const btn = doc.getElementById('login_sms');
                if (btn) btn.click();
                return {{ ok: true, submitted_sms: true }};
            }})()"#
        );
        let res = self.evaluate(&js).await?;
        tokio::time::sleep(Duration::from_millis(1800)).await;
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or_default())
    }
}

impl BrowserDriver for SteelBrowserDriver {
    async fn create_session(&self) -> AppResult<BrowserSessionInfo> {
        let url = format!("{}/v1/sessions", self.base_url);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            AppError::new(
                ErrorCode::SteelUnavailable,
                format!("Steel unreachable: {e}"),
            )
        })?;
        let val: serde_json::Value = resp.json().await.unwrap_or_default();
        if let Some(first) = val
            .get("sessions")
            .and_then(|s| s.as_array())
            .and_then(|a| a.first())
        {
            let id = first
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            return Ok(BrowserSessionInfo {
                session_id: id,
                status: first
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("live")
                    .to_string(),
                websocket_url: self.cdp_url.clone(),
                viewer_url: format!("{}/", self.base_url),
                debug_url: first
                    .get("debugUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }

        let create_resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| {
                AppError::new(
                    ErrorCode::SteelUnavailable,
                    format!("Steel session create error: {e}"),
                )
            })?;
        let created: serde_json::Value = create_resp.json().await.unwrap_or_default();
        let id = created
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("steel-session")
            .to_string();

        Ok(BrowserSessionInfo {
            session_id: id,
            status: "live".to_string(),
            websocket_url: self.cdp_url.clone(),
            viewer_url: format!("{}/", self.base_url),
            debug_url: Some(format!("{}/v1/sessions/debug", self.base_url)),
        })
    }

    async fn attach(&self, session_id: &str) -> AppResult<BrowserSessionInfo> {
        Ok(BrowserSessionInfo {
            session_id: session_id.to_string(),
            status: "attached".to_string(),
            websocket_url: self.cdp_url.clone(),
            viewer_url: format!("{}/", self.base_url),
            debug_url: Some(format!("{}/v1/sessions/debug", self.base_url)),
        })
    }

    async fn close_session(&self, _session_id: &str) -> AppResult<()> {
        Ok(())
    }

    async fn navigate(&self, url: &str) -> AppResult<BrowserSnapshot> {
        let session = self.create_session().await?;

        // 1. Check if authenticated cookies exist on disk, and inject them into Chromium via CDP
        let cookie_file = std::path::Path::new("/data/auth/sinopec_cookies.json");
        let local_cookie_file = std::path::Path::new("data/auth/sinopec_cookies.json");
        let path_to_use = if cookie_file.exists() {
            cookie_file
        } else {
            local_cookie_file
        };
        let disk_cookies = crate::sinopec::http::SinopecHttpTransport::load_cookies_from_disk(
            &path_to_use.to_path_buf(),
        );
        let has_authenticated_cookies =
            disk_cookies.contains_key("JSESSIONID") && disk_cookies.contains_key("LASTMSG");

        if !disk_cookies.is_empty() {
            let mut cookie_list = Vec::new();
            for (k, v) in &disk_cookies {
                cookie_list.push(serde_json::json!({
                    "name": k,
                    "value": v,
                    "domain": "www.sinopecsales.com",
                    "path": "/"
                }));
                cookie_list.push(serde_json::json!({
                    "name": k,
                    "value": v,
                    "domain": ".sinopecsales.com",
                    "path": "/"
                }));
            }
            let _ = self
                .run_cdp_command(
                    "Network.setCookies",
                    serde_json::json!({ "cookies": cookie_list }),
                )
                .await;
        }

        // 2. Navigate to URL
        let _ = self
            .run_cdp_command("Page.navigate", serde_json::json!({ "url": url }))
            .await?;
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // 3. If authenticated cookies exist, hide login popup so logged-in portal is shown!
        if has_authenticated_cookies {
            let _ = self
                .evaluate("if (typeof $ === 'function') { $('#mengbanbg,#login_win').hide(); }")
                .await;
        } else if url.contains("default_corp.html") || url.contains("sinopecsales.com") {
            let _ = self
                .evaluate("if (typeof showBg === 'function') { showBg('corp'); }")
                .await;
        }

        let page_url = self.get_url().await.unwrap_or_else(|_| url.to_string());
        let title = self
            .get_title()
            .await
            .unwrap_or_else(|_| "中国石化加油卡网上营业厅".to_string());
        let cookies = self.sync_sinopec_cookies().await.unwrap_or_default();
        let dom = self.get_dom().await.unwrap_or_default();
        let excerpt: String = dom.chars().take(400).collect();

        Ok(BrowserSnapshot {
            session_id: session.session_id,
            url: page_url,
            title,
            viewer_url: session.viewer_url,
            sinopec_cookie_count: cookies.len(),
            dom_excerpt: excerpt,
        })
    }

    async fn get_url(&self) -> AppResult<String> {
        let res = self.evaluate("window.location.href").await?;
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("https://www.sinopecsales.com/default_corp.html")
            .to_string())
    }

    async fn get_title(&self) -> AppResult<String> {
        let res = self.evaluate("document.title").await?;
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("中国石化加油卡网上营业厅")
            .to_string())
    }

    async fn get_dom(&self) -> AppResult<String> {
        let res = self
            .evaluate(
                "document.documentElement ? document.documentElement.outerHTML.slice(0, 2000) : ''",
            )
            .await?;
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string())
    }

    async fn evaluate(&self, expression: &str) -> AppResult<serde_json::Value> {
        self.run_cdp_command(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": expression,
                "returnByValue": true
            }),
        )
        .await
    }

    async fn click(&self, selector: &str) -> AppResult<()> {
        let js = format!(
            "(() => {{ const el = document.querySelector({:?}); if (el) el.click(); }})()",
            selector
        );
        let _ = self.evaluate(&js).await?;
        Ok(())
    }

    async fn fill(&self, selector: &str, value: &str) -> AppResult<()> {
        let js = format!(
            "(() => {{ const el = document.querySelector({:?}); if (el) {{ el.value = {:?}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); }} }})()",
            selector, value
        );
        let _ = self.evaluate(&js).await?;
        Ok(())
    }

    async fn cookies(&self) -> AppResult<Vec<BrowserCookie>> {
        let session = self.create_session().await?;
        let ctx_url = format!(
            "{}/v1/sessions/{}/context",
            self.base_url, session.session_id
        );
        if let Ok(resp) = self.client.get(&ctx_url).send().await {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = val.get("cookies").and_then(|c| c.as_array()) {
                    let out = arr
                        .iter()
                        .filter_map(|item| {
                            Some(BrowserCookie {
                                name: item.get("name")?.as_str()?.to_string(),
                                value: item.get("value")?.as_str()?.to_string(),
                                domain: item.get("domain")?.as_str()?.to_string(),
                                path: item
                                    .get("path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("/")
                                    .to_string(),
                                http_only: item
                                    .get("httpOnly")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                secure: item
                                    .get("secure")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                            })
                        })
                        .collect();
                    return Ok(out);
                }
            }
        }
        Ok(Vec::new())
    }

    async fn storage(&self) -> AppResult<HashMap<String, serde_json::Value>> {
        let session = self.create_session().await?;
        let ctx_url = format!(
            "{}/v1/sessions/{}/context",
            self.base_url, session.session_id
        );
        let mut out = HashMap::new();
        if let Ok(resp) = self.client.get(&ctx_url).send().await {
            if let Ok(val) = resp.json::<serde_json::Value>().await {
                if let Some(ls) = val.get("localStorage") {
                    out.insert("localStorage".to_string(), ls.clone());
                }
            }
        }
        Ok(out)
    }

    async fn screenshot(&self) -> AppResult<Option<String>> {
        let res = self
            .run_cdp_command(
                "Page.captureScreenshot",
                serde_json::json!({ "format": "png" }),
            )
            .await?;
        Ok(res
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()))
    }

    async fn start_network_capture(&self) -> AppResult<()> {
        let _ = self
            .run_cdp_command("Network.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn stop_network_capture(&self) -> AppResult<()> {
        let _ = self
            .run_cdp_command("Network.disable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn viewer(&self) -> AppResult<String> {
        Ok(format!("{}/", self.base_url))
    }
}
