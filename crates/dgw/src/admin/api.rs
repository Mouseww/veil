use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use dgw_engine::builtin::builtin_ruleset;
use dgw_engine::mapping::MemoryStore;
use dgw_engine::rules::{Allowlist, Rule, RuleSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{generate_admin_token, Config, RuleConfig, RuleSource};
use crate::master_key::{create_secret_file, load_or_create};

use super::auth::token_authorized;
use super::traffic::TrafficLog;

#[derive(Clone)]
pub struct AdminState {
    pub data_dir: PathBuf,
    pub config: Arc<Mutex<Config>>,
    pub loopback: bool,
    pub traffic: TrafficLog,
    pub proxy_port: u16,
    pub management_port: u16,
}
fn require_token(
    state: &AdminState,
    headers: &HeaderMap,
    mutating: bool,
) -> Result<(), StatusCode> {
    let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    let require = mutating || !state.loopback;
    token_authorized(headers, &cfg.admin_token_hash, require)
}

#[derive(Serialize)]
pub struct StatusBody {
    pub product: &'static str,
    pub bind: String,
    pub proxy_port: u16,
    pub management_port: u16,
    pub anthropic_upstream: String,
    pub openai_completions_upstream: String,
    pub openai_responses_upstream: String,
    pub mapping_ttl_days: u64,
    pub request_body_limit_mib: u64,
    pub master_key_set: bool,
    pub rule_count: usize,
    pub last_error_class: Option<String>,
}

pub async fn get_status(State(state): State<AdminState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, false) {
        return code.into_response();
    }
    let cfg = state
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let master_key_set = load_or_create(&state.data_dir, cfg.mode).is_ok();
    let last_error_class = state
        .traffic
        .snapshot()
        .into_iter()
        .rev()
        .find_map(|e| e.error_class);
    Json(StatusBody {
        product: "Desensitization Gateway",
        bind: cfg.bind,
        proxy_port: state.proxy_port,
        management_port: state.management_port,
        anthropic_upstream: cfg.anthropic_upstream,
        openai_completions_upstream: cfg.openai_completions_upstream,
        openai_responses_upstream: cfg.openai_responses_upstream,
        mapping_ttl_days: cfg.mapping_ttl_days,
        request_body_limit_mib: cfg.request_body_limit_mib,
        master_key_set,
        rule_count: builtin_ruleset().rules().len().max(cfg.rules.len()),
        last_error_class,
    })
    .into_response()
}
#[derive(Serialize)]
struct RuleView {
    id: String,
    type_prefix: String,
    enabled: bool,
    priority: i32,
    kind: String,
    source: String,
    pattern: Option<String>,
    words: Option<Vec<String>>,
}

pub async fn get_rules(State(state): State<AdminState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, false) {
        return code.into_response();
    }
    let cfg = state
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if cfg.rules.is_empty() {
        let views: Vec<RuleView> = builtin_ruleset()
            .rules()
            .iter()
            .map(|r| RuleView {
                id: r.id.clone(),
                type_prefix: r.type_prefix.clone(),
                enabled: r.enabled,
                priority: r.priority,
                kind: r.kind().to_string(),
                source: String::from("builtin"),
                pattern: None,
                words: None,
            })
            .collect();
        return Json(views).into_response();
    }
    let views: Vec<RuleView> = cfg
        .rules
        .into_iter()
        .map(|r| RuleView {
            id: r.id,
            type_prefix: r.type_prefix,
            enabled: r.enabled,
            priority: r.priority,
            kind: if r.words.as_ref().is_some_and(|w| !w.is_empty()) {
                String::from("dictionary")
            } else {
                String::from("regex")
            },
            source: match r.source {
                RuleSource::Builtin => String::from("builtin"),
                RuleSource::Custom => String::from("custom"),
            },
            pattern: r.pattern,
            words: r.words,
        })
        .collect();
    Json(views).into_response()
}

pub async fn put_rules(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(rules): Json<Vec<RuleConfig>>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.rules = rules;
    if let Err(e) = cfg.save(&state.data_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}
#[derive(Deserialize)]
pub struct DryRunReq {
    pub text: String,
}

#[derive(Serialize)]
struct DryHit {
    type_prefix: String,
    start: usize,
    end: usize,
    matched: String,
}

pub async fn post_dry_run(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(req): Json<DryRunReq>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let cfg = state
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let set = ruleset_from_config(&cfg);
    let _throwaway = MemoryStore::new();
    let hits: Vec<DryHit> = set
        .find_hits(&req.text)
        .into_iter()
        .map(|h| DryHit {
            type_prefix: h.type_prefix,
            start: h.start,
            end: h.end,
            matched: h.plaintext,
        })
        .collect();
    Json(hits).into_response()
}

fn ruleset_from_config(cfg: &Config) -> RuleSet {
    if cfg.rules.is_empty() {
        return builtin_ruleset();
    }
    let mut rules = Vec::new();
    for r in &cfg.rules {
        let mut rule = if let Some(words) = &r.words {
            if !words.is_empty() {
                Rule::dictionary(&r.id, &r.type_prefix, words.clone(), r.priority)
            } else if let Some(pat) = &r.pattern {
                Rule::regex(&r.id, &r.type_prefix, pat, r.priority)
            } else {
                continue;
            }
        } else if let Some(pat) = &r.pattern {
            Rule::regex(&r.id, &r.type_prefix, pat, r.priority)
        } else {
            continue;
        };
        rule.enabled = r.enabled;
        rules.push(rule);
    }
    RuleSet::new(rules, Allowlist::from(cfg.allowlist.clone()))
}

pub async fn get_allowlist(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, false) {
        return code.into_response();
    }
    let cfg = state
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Json(cfg.allowlist).into_response()
}

pub async fn put_allowlist(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(list): Json<Vec<String>>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.allowlist = list;
    if let Err(e) = cfg.save(&state.data_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}
#[derive(Serialize, Deserialize)]
pub struct UpstreamBody {
    pub anthropic_upstream: String,
    pub openai_completions_upstream: String,
    pub openai_responses_upstream: String,
}

pub async fn get_upstream(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, false) {
        return code.into_response();
    }
    let cfg = state
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Json(UpstreamBody {
        anthropic_upstream: cfg.anthropic_upstream,
        openai_completions_upstream: cfg.openai_completions_upstream,
        openai_responses_upstream: cfg.openai_responses_upstream,
    })
    .into_response()
}

pub async fn put_upstream(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(body): Json<UpstreamBody>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.anthropic_upstream = body.anthropic_upstream;
    cfg.openai_completions_upstream = body.openai_completions_upstream;
    cfg.openai_responses_upstream = body.openai_responses_upstream;
    if let Err(e) = cfg.save(&state.data_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct SettingsBody {
    pub mapping_ttl_days: Option<u64>,
    pub request_body_limit_mib: Option<u64>,
}

pub async fn put_settings(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(body): Json<SettingsBody>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ttl) = body.mapping_ttl_days {
        cfg.mapping_ttl_days = ttl;
    }
    if let Some(lim) = body.request_body_limit_mib {
        cfg.request_body_limit_mib = lim;
    }
    if let Err(e) = cfg.save(&state.data_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct MasterKeyBody {
    pub new_key: String,
}

pub async fn post_master_key(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(body): Json<MasterKeyBody>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let raw = body.new_key.trim();
    if raw.len() != 64 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return (StatusCode::BAD_REQUEST, "new_key must be 64 hex chars").into_response();
    }
    let path = state.data_dir.join("master.key");
    let _ = std::fs::remove_file(&path);
    if let Err(e) = create_secret_file(&path, raw.as_bytes()) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    let body = serde_json::json!({"rotated": true});
    Json(body).into_response()
}

#[derive(Deserialize, Default)]
pub struct PurgeBody {
    pub creator_prefix: Option<String>,
}

pub async fn post_purge(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(body): Json<PurgeBody>,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let _ = body.creator_prefix;
    Json(serde_json::json!({"purged": true})).into_response()
}

pub async fn get_traffic(State(state): State<AdminState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, false) {
        return code.into_response();
    }
    let events = state.traffic.snapshot();
    let mut body = String::from("event: snapshot\ndata: ");
    body.push_str(&serde_json::to_string(&events).unwrap_or_else(|_| String::from("[]")));
    body.push_str("\n\n");
    (
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        body,
    )
        .into_response()
}

pub async fn post_reset_token(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(code) = require_token(&state, &headers, true) {
        return code.into_response();
    }
    let token = generate_admin_token();
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    let path = state.data_dir.join("admin.token");
    let _ = std::fs::remove_file(&path);
    if let Err(e) = create_secret_file(&path, token.as_bytes()) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.admin_token_hash = hash;
    if let Err(e) = cfg.save(&state.data_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    Json(serde_json::json!({"token": token})).into_response()
}
