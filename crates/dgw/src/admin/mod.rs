mod api;
mod auth;
mod traffic;

pub use api::AdminState;
pub use traffic::{TrafficEvent, TrafficLog};

use axum::routing::{get, post, put};
use axum::Router;

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
        .with_state(state)
}
