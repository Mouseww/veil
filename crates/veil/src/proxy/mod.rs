mod classify;
mod forward;
mod sse;

pub use classify::{classify, ProtocolFamily, UpstreamConfig};
pub use forward::{proxy_handler, AppState};
pub use sse::{restore_json_body, SseRestorer};

use axum::routing::any;
use axum::Router;

pub fn router(state: AppState) -> Router {
    Router::new().fallback(any(proxy_handler)).with_state(state)
}
