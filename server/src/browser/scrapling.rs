use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraplingStatus {
    pub configured: bool,
    pub reachable: bool,
    pub base_url: Option<String>,
    pub auth_required: bool,
    pub role: String,
}

#[derive(Clone)]
pub struct ScraplingClient {
    base_url: Option<String>,
    client: reqwest::Client,
}

impl ScraplingClient {
    pub fn new(base_url: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self { base_url, client }
    }

    pub async fn status(&self) -> ScraplingStatus {
        let Some(base) = &self.base_url else {
            return ScraplingStatus {
                configured: false,
                reachable: false,
                base_url: None,
                auth_required: false,
                role: "AI Research Assistant (disabled in production)".to_string(),
            };
        };

        let mcp_url = format!("{}/mcp", base.trim_end_matches('/'));
        match self.client.get(&mcp_url).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                ScraplingStatus {
                    configured: true,
                    reachable: true,
                    base_url: Some(base.clone()),
                    auth_required: code == 401 || code == 403,
                    role: "AI Research Assistant (optional; closing Scrapling does not affect sinopec-server)".to_string(),
                }
            }
            Err(_) => ScraplingStatus {
                configured: true,
                reachable: false,
                base_url: Some(base.clone()),
                auth_required: false,
                role: "AI Research Assistant (offline; production unaffected)".to_string(),
            },
        }
    }
}
