use std::sync::atomic::{AtomicBool, Ordering};

use super::api::{require_token, AdminState};
use crate::config::Mode;
use crate::local_identity::{self, GeneratedRule, Selection};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};

#[derive(Clone)]
pub(super) struct RunningIdentity(pub local_identity::IdentityConfig);

static COLLECTING: AtomicBool = AtomicBool::new(false);
struct CollectionGuard;
impl Drop for CollectionGuard {
    fn drop(&mut self) {
        COLLECTING.store(false, Ordering::Release);
    }
}

pub async fn get_identity(
    State(state): State<AdminState>,
    Extension(running): Extension<RunningIdentity>,
    headers: HeaderMap,
) -> Response {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    Json(cfg.local_identity.view(&running.0)).into_response()
}

pub async fn put_identity(
    State(state): State<AdminState>,
    Extension(running): Extension<RunningIdentity>,
    headers: HeaderMap,
    Json(selection): Json<Selection>,
) -> Response {
    change(state, running, headers, Some(selection)).await
}
pub async fn refresh_identity(
    State(state): State<AdminState>,
    Extension(running): Extension<RunningIdentity>,
    headers: HeaderMap,
) -> Response {
    change(state, running, headers, None).await
}

fn missing_category(
    selection: &Selection,
    incoming: &[GeneratedRule],
    saved: &[GeneratedRule],
) -> bool {
    let categories = ["app_ids", "hardware", "user_context", "git_email"];
    !categories
        .iter()
        .any(|category| selection.includes(category))
        || categories.into_iter().any(|category| {
            selection.includes(category)
                && !incoming
                    .iter()
                    .chain(saved.iter())
                    .any(|rule| rule.category == category)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_other_category_cannot_hide_missing_selected_category() {
        let mut selection = Selection::default();
        selection.enabled = true;
        let mut values = local_identity::Collected::default();
        values.device_ids.push("ab".repeat(32));
        let app_rules = local_identity::generate(&values, &selection);
        assert!(missing_category(&selection, &[], &app_rules));
        selection.hardware = false;
        assert!(!missing_category(&selection, &[], &app_rules));
        selection.app_ids = false;
        selection.hardware = true;
        assert!(missing_category(&selection, &[], &app_rules));
        values
            .machine_guid
            .push("22345678-1234-4321-abcd-9876543210ab".into());
        let hardware = local_identity::generate(&values, &selection);
        assert!(!missing_category(&selection, &hardware, &app_rules));
        assert!(!missing_category(&selection, &[], &hardware));
        selection.hardware = false;
        assert!(missing_category(&selection, &hardware, &app_rules));
    }
}

async fn change(
    state: AdminState,
    running: RunningIdentity,
    headers: HeaderMap,
    requested: Option<Selection>,
) -> Response {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let selection = {
        let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.mode != Mode::Desktop {
            return (
                StatusCode::BAD_REQUEST,
                "local collection is only available in desktop mode",
            )
                .into_response();
        }
        requested.unwrap_or_else(|| cfg.local_identity.selection.clone())
    };
    if COLLECTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return (StatusCode::CONFLICT, "local collection already in progress").into_response();
    }
    let guard = CollectionGuard;
    let result = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        let incoming = if selection.enabled {
            let values = local_identity::collect(&selection);
            local_identity::generate(&values, &selection)
        } else {
            Vec::new()
        };
        let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = cfg.clone();
        if selection.enabled && missing_category(&selection, &incoming, &next.local_identity.rules)
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "select categories with collected or saved identifiers; existing settings are unchanged",
            ));
        }
        if selection.enabled {
            next.local_identity.merge(incoming).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "local identity history is full; settings are unchanged",
                )
            })?;
        }
        next.local_identity.selection = selection;
        next.save(&state.data_dir).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not save local protection settings",
            )
        })?;
        let view = next.local_identity.view(&running.0);
        *cfg = next;
        Ok(view)
    })
    .await;
    match result {
        Ok(Ok(view)) => Json(view).into_response(),
        Ok(Err((code, message))) => (code, message).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "local collection failed; refresh the settings to check the saved state",
        )
            .into_response(),
    }
}
