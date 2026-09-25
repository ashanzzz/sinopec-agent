use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSessionInfo {
    pub session_id: String,
    pub status: String,
    pub websocket_url: String,
    pub viewer_url: String,
    pub debug_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSnapshot {
    pub session_id: String,
    pub url: String,
    pub title: String,
    pub viewer_url: String,
    pub sinopec_cookie_count: usize,
    pub dom_excerpt: String,
}

#[allow(async_fn_in_trait)]
pub trait BrowserDriver: Send + Sync {
    async fn create_session(&self) -> AppResult<BrowserSessionInfo>;
    async fn attach(&self, session_id: &str) -> AppResult<BrowserSessionInfo>;
    async fn close_session(&self, session_id: &str) -> AppResult<()>;
    async fn navigate(&self, url: &str) -> AppResult<BrowserSnapshot>;
    async fn get_url(&self) -> AppResult<String>;
    async fn get_title(&self) -> AppResult<String>;
    async fn get_dom(&self) -> AppResult<String>;
    async fn evaluate(&self, expression: &str) -> AppResult<serde_json::Value>;
    async fn click(&self, selector: &str) -> AppResult<()>;
    async fn fill(&self, selector: &str, value: &str) -> AppResult<()>;
    async fn cookies(&self) -> AppResult<Vec<BrowserCookie>>;
    async fn storage(&self) -> AppResult<HashMap<String, serde_json::Value>>;
    async fn screenshot(&self) -> AppResult<Option<String>>;
    async fn start_network_capture(&self) -> AppResult<()>;
    async fn stop_network_capture(&self) -> AppResult<()>;
    async fn viewer(&self) -> AppResult<String>;
}
