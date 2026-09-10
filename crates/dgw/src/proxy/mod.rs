mod classify;
mod forward;

pub use classify::{classify, ProtocolFamily, UpstreamConfig};
pub use forward::{proxy_handler, AppState};

use axum::routing::any;
use axum::Router;

pub fn router(state: AppState) -> Router {
    Router::new().fallback(any(proxy_handler)).with_state(state)
}
