use crate::auth::AuthManager;
use crate::browser::{ScraplingClient, SteelBrowserDriver};
use crate::config::AppConfig;
use crate::downloads::DownloadManager;
use crate::error::AppResult;
use crate::human::HumanActionManager;
use crate::research::ResearchRecorder;
use crate::sinopec::{http::SinopecHttpTransport, service::SinopecService};
use crate::storage::SqliteStore;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub store: SqliteStore,
    pub auth: AuthManager,
    pub http: SinopecHttpTransport,
    pub browser: SteelBrowserDriver,
    pub scrapling: ScraplingClient,
    pub human: HumanActionManager,
    pub downloads: DownloadManager,
    pub recorder: ResearchRecorder,
    pub service: SinopecService,
}

impl AppState {
    pub async fn initialize(config: AppConfig) -> AppResult<Self> {
        config.ensure_directories()?;
        let store = SqliteStore::connect(&config.db_path()).await?;
        let recorder = ResearchRecorder::new(
            config.research_dir(),
            config.research_mode,
            config.demo_mode,
        );
        let cookie_file = config.auth_dir().join("sinopec_cookies.json");
        let http = SinopecHttpTransport::new(
            config.sinopec_base_url.clone(),
            cookie_file,
            recorder.clone(),
        );
        let credentials = crate::auth::credentials::CredentialProvider::new(config.secrets_dir());
        let verifier = crate::auth::verifier::AuthVerifier::new(http.clone());
        let auth = AuthManager::new(
            credentials,
            verifier,
            config.auth_dir(),
            config.max_auth_attempts,
        );
        let browser =
            SteelBrowserDriver::new(config.steel_base_url.clone(), config.steel_cdp_url.clone());
        let scrapling = ScraplingClient::new(config.scrapling_base_url.clone());
        let human = HumanActionManager::new(
            store.clone(),
            config.public_base_url.clone(),
            format!("{}/", config.steel_base_url.trim_end_matches('/')),
        );
        let downloads = DownloadManager::new(store.clone(), config.downloads_dir());
        let service = SinopecService::new(
            config.clone(),
            store.clone(),
            auth.clone(),
            http.clone(),
            browser.clone(),
            human.clone(),
            downloads.clone(),
            recorder.clone(),
        );

        Ok(Self {
            config,
            store,
            auth,
            http,
            browser,
            scrapling,
            human,
            downloads,
            recorder,
            service,
        })
    }
}
