use axum::{
    extract::{Path, State},
    response::Html,
};

use crate::error::AppResult;
use crate::state::AppState;

pub async fn render_human_action_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Html<String>> {
    let action = state.human.get_by_id_or_token(&token).await?;
    let stored = state.auth.credentials().load()?.unwrap_or_default();
    let auth_status = state.service.auth_status().await?;
    let viewer = action
        .viewer_url
        .clone()
        .unwrap_or_else(|| format!("{}/", state.browser.base_url()));

    let tax_code = stored
        .tax_code
        .unwrap_or_else(|| "91120116MA06ABCDEF".to_string());
    let id_number = stored
        .id_number
        .unwrap_or_else(|| "120101199001011234".to_string());
    let holder_name = stored.holder_name.unwrap_or_else(|| "张*山".to_string());
    let phone = stored.phone.unwrap_or_else(|| "13800138000".to_string());
    let initial_logged_in = if auth_status.verified {
        "true"
    } else {
        "false"
    };

    let template_owned = std::fs::read_to_string("server/src/api/human_ui_template.html")
        .unwrap_or_else(|_| include_str!("human_ui_template.html").to_string());
    let template = template_owned.as_str();
    let html = template
        .replace("__VIEWER__", &viewer)
        .replace("__TAX_CODE__", &tax_code)
        .replace("__ID_NUMBER__", &id_number)
        .replace("__HOLDER_NAME__", &holder_name)
        .replace("__PHONE__", &phone)
        .replace("__INITIAL_LOGGED_IN__", initial_logged_in)
        .replace(
            "__BODY_CLASS__",
            if auth_status.verified {
                ""
            } else {
                "not-logged-in"
            },
        )
        .replace("__ACTION_ID__", &action.id);

    Ok(Html(html))
}
