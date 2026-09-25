use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Auto,
    Manual,
    Hybrid,
}

impl AuthMode {
    pub fn from_env(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "auto" => Self::Auto,
            "manual" => Self::Manual,
            _ => Self::Hybrid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind_addr: String,
    pub public_base_url: String,
    pub data_dir: PathBuf,
    pub api_token: Option<String>,
    pub auth_mode: AuthMode,
    pub auto_login: bool,
    pub max_auth_attempts: u32,
    pub research_mode: bool,
    pub demo_mode: bool,
    pub allow_auto_submit: bool,
    pub steel_base_url: String,
    pub steel_cdp_url: String,
    pub scrapling_base_url: Option<String>,
    pub sinopec_base_url: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let bind_addr = env::var("SINOPEC_BIND").unwrap_or_else(|_| "0.0.0.0:8788".to_string());
        let public_base_url = env::var("SINOPEC_PUBLIC_URL").unwrap_or_else(|_| {
            format!(
                "http://127.0.0.1:{}",
                bind_addr.split(':').next_back().unwrap_or("8788")
            )
        });
        let data_dir = env::var("SINOPEC_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let api_token = env::var("SINOPEC_API_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let auth_mode = env::var("SINOPEC_AUTH_MODE")
            .map(|v| AuthMode::from_env(&v))
            .unwrap_or(AuthMode::Hybrid);
        let auto_login = env_bool("SINOPEC_AUTO_LOGIN", true);
        let max_auth_attempts = env::var("SINOPEC_MAX_AUTH_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3)
            .max(1);
        let research_mode = env_bool("SINOPEC_RESEARCH_MODE", false);
        let demo_mode = env_bool("SINOPEC_DEMO_MODE", false);
        let allow_auto_submit = env_bool("SINOPEC_ALLOW_AUTO_SUBMIT", false);
        let steel_base_url =
            env::var("STEEL_BASE_URL").unwrap_or_else(|_| "http://192.168.8.11:13000".to_string());
        let steel_cdp_url =
            env::var("STEEL_CDP_URL").unwrap_or_else(|_| "ws://192.168.8.11:19223".to_string());
        let scrapling_base_url = env::var("SCRAPLING_BASE_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some("http://192.168.8.11:8111".to_string()));
        let sinopec_base_url = env::var("SINOPEC_BASE_URL")
            .unwrap_or_else(|_| "https://www.sinopecsales.com".to_string());

        Self {
            bind_addr,
            public_base_url,
            data_dir,
            api_token,
            auth_mode,
            auto_login,
            max_auth_attempts,
            research_mode,
            demo_mode,
            allow_auto_submit,
            steel_base_url,
            steel_cdp_url,
            scrapling_base_url,
            sinopec_base_url,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("sinopec.db")
    }

    pub fn secrets_dir(&self) -> PathBuf {
        self.data_dir.join("secrets")
    }

    pub fn auth_dir(&self) -> PathBuf {
        self.data_dir.join("auth")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir.join("downloads")
    }

    pub fn research_dir(&self) -> PathBuf {
        self.data_dir.join("research")
    }

    pub fn ensure_directories(&self) -> std::io::Result<()> {
        for dir in [
            self.data_dir.clone(),
            self.secrets_dir(),
            self.auth_dir(),
            self.downloads_dir(),
            self.data_dir.join("logs"),
            self.research_dir().join("screenshots"),
            self.research_dir().join("pages"),
            self.research_dir().join("network"),
            self.research_dir().join("responses"),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(val) => matches!(
            val.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}
