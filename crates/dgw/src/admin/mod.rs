mod api;
mod auth;
mod traffic;

pub use api::AdminState;
pub use traffic::{TrafficEvent, TrafficLog};

use axum::http::{header, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::Router;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
struct Dist;

pub fn router(state: AdminState) -> Router {
    Router::new()
        .route("/api/status", get(api::get_status))
        .route("/api/rules", get(api::get_rules).put(api::put_rules))
        .route("/api/rules/dry-run", post(api::post_dry_run))
        .route("/api/allowlist", get(api::get_allowlist).put(api::put_allowlist))
        .route("/api/upstream", get(api::get_upstream).put(api::put_upstream))
        .route("/api/settings", put(api::put_settings))
        .route("/api/master-key", post(api::post_master_key))
        .route("/api/mappings/purge", post(api::post_purge))
        .route("/api/traffic", get(api::get_traffic))
        .route("/api/admin-token/reset", post(api::post_reset_token))
        .fallback(get(static_file))
        .with_state(state)
}

async fn static_file(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Dist::get(path).or_else(|| Dist::get("index.html"));
    let Some(file) = file else {
        return (StatusCode::NOT_FOUND, "ui not built").into_response();
    };
    let mime = if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "text/html; charset=utf-8"
    };
    ([(header::CONTENT_TYPE, mime)], file.data.into_owned()).into_response()
}
