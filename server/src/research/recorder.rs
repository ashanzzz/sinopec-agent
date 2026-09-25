use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::AppResult;
use crate::research::redactor::Redactor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEventRecord {
    pub id: String,
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub timestamp: String,
    pub category: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchStatusSnapshot {
    pub research_mode: bool,
    pub demo_mode: bool,
    pub current_page_url: String,
    pub current_page_title: String,
    pub auth_state: String,
    pub confirmed_endpoints_count: usize,
    pub hypothesis_endpoints_count: usize,
    pub unknown_endpoints_count: usize,
    pub recent_xhr: Vec<NetworkEventRecord>,
    pub recent_errors: Vec<String>,
    pub latest_screenshot: Option<String>,
    pub timeline: Vec<TimelineEvent>,
}

struct InnerState {
    current_page_url: String,
    current_page_title: String,
    recent_xhr: VecDeque<NetworkEventRecord>,
    recent_errors: VecDeque<String>,
    latest_screenshot: Option<String>,
    timeline: VecDeque<TimelineEvent>,
}

#[derive(Clone)]
pub struct ResearchRecorder {
    base_dir: PathBuf,
    research_mode: bool,
    demo_mode: bool,
    inner: Arc<RwLock<InnerState>>,
}

impl ResearchRecorder {
    pub fn new(base_dir: PathBuf, research_mode: bool, demo_mode: bool) -> Self {
        let mut timeline = VecDeque::new();
        timeline.push_back(TimelineEvent {
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            category: "INIT".to_string(),
            summary: "Initialized ResearchRecorder & loaded Sinopec protocol knowledge base"
                .to_string(),
        });
        timeline.push_back(TimelineEvent {
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            category: "DISCOVERY".to_string(),
            summary: "Confirmed default_corp.html -> /corpgas/res/html/login/login_pc.jsp and HTTP 390 session check".to_string(),
        });

        Self {
            base_dir,
            research_mode,
            demo_mode,
            inner: Arc::new(RwLock::new(InnerState {
                current_page_url: "https://www.sinopecsales.com/default_corp.html".to_string(),
                current_page_title: "中国石化加油卡网上营业厅".to_string(),
                recent_xhr: VecDeque::new(),
                recent_errors: VecDeque::new(),
                latest_screenshot: None,
                timeline,
            })),
        }
    }

    pub async fn record_timeline(&self, category: &str, summary: impl Into<String>) {
        let redacted = Redactor::redact_text(&summary.into());
        let mut guard = self.inner.write().await;
        guard.timeline.push_front(TimelineEvent {
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            category: category.to_string(),
            summary: redacted,
        });
        while guard.timeline.len() > 60 {
            guard.timeline.pop_back();
        }
    }

    pub async fn update_page(&self, url: impl Into<String>, title: impl Into<String>) {
        let mut guard = self.inner.write().await;
        guard.current_page_url = Redactor::redact_text(&url.into());
        guard.current_page_title = title.into();
    }

    pub async fn record_network_event(
        &self,
        method: &str,
        url: &str,
        status: Option<u16>,
        content_type: Option<String>,
        kind: &str,
        payload: Option<&serde_json::Value>,
    ) -> AppResult<()> {
        let clean_url = Redactor::redact_text(url);
        let event = NetworkEventRecord {
            id: format!("net_{}", Uuid::new_v4().simple()),
            timestamp: Utc::now().to_rfc3339(),
            method: method.to_uppercase(),
            url: clean_url.clone(),
            status,
            content_type,
            kind: kind.to_string(),
        };

        {
            let mut guard = self.inner.write().await;
            guard.recent_xhr.push_front(event.clone());
            while guard.recent_xhr.len() > 50 {
                guard.recent_xhr.pop_back();
            }
        }

        if self.research_mode {
            let net_dir = self.base_dir.join("network");
            std::fs::create_dir_all(&net_dir)?;
            let doc = serde_json::json!({
                "event": event,
                "payload": payload.map(Redactor::redact_json),
            });
            let path = net_dir.join(format!("{}.json", event.id));
            std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default())?;
        }
        Ok(())
    }

    pub async fn record_response_fixture(
        &self,
        name: &str,
        body: &serde_json::Value,
    ) -> AppResult<PathBuf> {
        let redacted = Redactor::redact_json(body);
        let resp_dir = self.base_dir.join("responses");
        std::fs::create_dir_all(&resp_dir)?;
        let safe_name = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let file_path = resp_dir.join(format!("{safe_name}.json"));
        std::fs::write(
            &file_path,
            serde_json::to_string_pretty(&redacted).unwrap_or_default(),
        )?;
        Ok(file_path)
    }

    pub async fn record_error(&self, err_msg: impl Into<String>) {
        let clean = Redactor::redact_text(&err_msg.into());
        let mut guard = self.inner.write().await;
        guard.recent_errors.push_front(clean);
        while guard.recent_errors.len() > 25 {
            guard.recent_errors.pop_back();
        }
    }

    pub async fn snapshot(
        &self,
        auth_state: &str,
        confirmed_count: usize,
        hypothesis_count: usize,
        unknown_count: usize,
    ) -> ResearchStatusSnapshot {
        let guard = self.inner.read().await;
        ResearchStatusSnapshot {
            research_mode: self.research_mode,
            demo_mode: self.demo_mode,
            current_page_url: guard.current_page_url.clone(),
            current_page_title: guard.current_page_title.clone(),
            auth_state: auth_state.to_string(),
            confirmed_endpoints_count: confirmed_count,
            hypothesis_endpoints_count: hypothesis_count,
            unknown_endpoints_count: unknown_count,
            recent_xhr: guard.recent_xhr.iter().cloned().collect(),
            recent_errors: guard.recent_errors.iter().cloned().collect(),
            latest_screenshot: guard.latest_screenshot.clone(),
            timeline: guard.timeline.iter().cloned().collect(),
        }
    }
}
