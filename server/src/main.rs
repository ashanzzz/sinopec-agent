use sinopec_server::{
    api::routes::build_router, config::AppConfig, state::AppState, telemetry::init_tracing,
};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = AppConfig::from_env();

    if config.bind_addr.starts_with("0.0.0.0") && config.api_token.is_none() {
        warn!(
            bind = %config.bind_addr,
            "SINOPEC_API_TOKEN is empty while binding to 0.0.0.0. Set SINOPEC_API_TOKEN in production."
        );
    }

    let state = AppState::initialize(config.clone())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize sinopec-server state: {e}"))?;

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    info!(
        bind = %config.bind_addr,
        research_mode = config.research_mode,
        allow_auto_submit = config.allow_auto_submit,
        "sinopec-server listening (REST /api/v1 + MCP /mcp + Human UI /human/{{token}})"
    );
    axum::serve(listener, app).await?;
    Ok(())
}
