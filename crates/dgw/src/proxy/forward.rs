use std::sync::Arc;

use super::sse::{restore_json_body, SseRestorer};
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use dgw_engine::builtin::builtin_ruleset;
use dgw_engine::creator::creator_from_headers;
use dgw_engine::mapping::MappingStore;
use dgw_engine::rules::RuleSet;
use dgw_engine::walk::{desensitize_json, WalkError};
use http_body_util::BodyExt;
use serde_json::Value;

use super::classify::{classify, upstream_base, UpstreamConfig};

#[derive(Clone)]
pub struct AppState {
    pub upstream: UpstreamConfig,
    pub body_limit_bytes: usize,
    pub store: Arc<dyn MappingStore + Send + Sync>,
    pub rules: Arc<RuleSet>,
    pub client: reqwest::Client,
}

impl AppState {
    pub fn new(
        store: Arc<dyn MappingStore + Send + Sync>,
        upstream: UpstreamConfig,
        body_limit_bytes: usize,
    ) -> Self {
        Self {
            upstream,
            body_limit_bytes,
            store,
            rules: Arc::new(builtin_ruleset()),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("reqwest client"),
        }
    }
}

pub async fn proxy_handler(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
) -> Response {
    match proxy_inner(&state, req).await {
        Ok(resp) => resp,
        Err(resp) => resp,
    }
}

async fn proxy_inner(
    state: &AppState,
    req: axum::http::Request<Body>,
) -> Result<Response, Response> {
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let method = req.method().clone();
    let headers = req.headers().clone();

    let Some(family) = classify(&path) else {
        return Err((StatusCode::NOT_FOUND, "not found").into_response());
    };

    if is_multipart(&headers) {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "multipart rejected").into_response());
    }

    if let Some(len) = content_length(&headers) {
        if len > state.body_limit_bytes {
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "request too large").into_response());
        }
    }

    let raw = req
        .into_body()
        .collect()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "body").into_response())?
        .to_bytes();

    if raw.len() > state.body_limit_bytes {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "request too large").into_response());
    }

    let creator = creator_from_headers(&headers);
    let body = prepare_body(state, &headers, &raw, &creator)?;

    let base = upstream_base(family, &state.upstream).trim_end_matches('/');
    let mut url = String::new();
    url.push_str(base);
    url.push_str(&path);
    if let Some(q) = query {
        url.push('?');
        url.push_str(&q);
    }

    let mut builder = state.client.request(method, &url);
    for (name, value) in headers.iter() {
        if skip_request_header(name) {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder = builder.header(header::ACCEPT_ENCODING, "identity");
    if let Ok(host) = host_from_url(&url) {
        builder = builder.header(header::HOST, host);
    }
    builder = builder.body(body);

    let upstream = builder
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "upstream error").into_response())?;

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    for (name, value) in upstream.headers().iter() {
        if skip_response_header(name) {
            continue;
        }
        response = response.header(name, value);
    }
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = upstream
        .bytes()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "upstream body").into_response())?;
    let out = restore_response(state, &headers, &content_type, &bytes)?;
    response
        .body(Body::from(out))
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "response").into_response())
}

fn prepare_body(
    state: &AppState,
    headers: &HeaderMap,
    raw: &Bytes,
    creator: &dgw_engine::creator::Creator,
) -> Result<Bytes, Response> {
    if raw.is_empty() {
        return Ok(Bytes::new());
    }
    if looks_like_json(headers, raw) {
        let value: Value = serde_json::from_slice(raw)
            .map_err(|_| (StatusCode::BAD_GATEWAY, "unscannable json").into_response())?;
        match desensitize_json(&value, &state.rules, state.store.as_ref(), creator) {
            Ok(redacted) => {
                let out = serde_json::to_vec(&redacted)
                    .map_err(|_| (StatusCode::BAD_GATEWAY, "re-encode").into_response())?;
                Ok(Bytes::from(out))
            }
            Err(WalkError::MappingWrite) | Err(WalkError::StoreUnavailable) => {
                Err((StatusCode::BAD_GATEWAY, "desensitize failed").into_response())
            }
        }
    } else {
        let text = String::from_utf8_lossy(raw);
        let hits = state.rules.find_hits(&text);
        if hits.is_empty() {
            return Ok(raw.clone());
        }
        let mut out = text.into_owned();
        for hit in hits.into_iter().rev() {
            let placeholder = state
                .store
                .get_or_insert(creator, &hit.type_prefix, &hit.plaintext)
                .map_err(|_| (StatusCode::BAD_GATEWAY, "desensitize failed").into_response())?;
            out.replace_range(hit.start..hit.end, &placeholder.format());
        }
        Ok(Bytes::from(out))
    }
}

fn looks_like_json(headers: &HeaderMap, raw: &Bytes) -> bool {
    if let Some(ct) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        if ct.to_ascii_lowercase().contains("json") {
            return true;
        }
    }
    match raw.iter().copied().find(|b| !b.is_ascii_whitespace()) {
        Some(b) if b == 123 || b == 91 => true,
        _ => false,
    }
}

fn is_multipart(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.to_ascii_lowercase().starts_with("multipart/"))
}

fn content_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

fn skip_request_header(name: &header::HeaderName) -> bool {
    name == header::HOST
        || name == header::ACCEPT_ENCODING
        || name == header::CONTENT_LENGTH
        || name == header::TRANSFER_ENCODING
        || name == header::CONNECTION
}

fn skip_response_header(name: &header::HeaderName) -> bool {
    name == header::TRANSFER_ENCODING
        || name == header::CONNECTION
        || name == header::CONTENT_LENGTH
}

fn host_from_url(url: &str) -> Result<HeaderValue, ()> {
    let u = reqwest::Url::parse(url).map_err(|_| ())?;
    let host = u.host_str().ok_or(())?;
    let value = match u.port() {
        Some(port) => {
            let mut s = String::from(host);
            s.push(':');
            s.push_str(&port.to_string());
            s
        }
        None => host.to_string(),
    };
    HeaderValue::from_str(&value).map_err(|_| ())
}

fn restore_response(
    state: &AppState,
    req_headers: &HeaderMap,
    content_type: &str,
    bytes: &Bytes,
) -> Result<Bytes, Response> {
    let creator = creator_from_headers(req_headers);
    if content_type.contains("text/event-stream") {
        let text = String::from_utf8_lossy(bytes);
        let mut restorer = SseRestorer::new(state.store.as_ref(), creator);
        let restored = restorer.push(&text).and_then(|head| {
            let tail = restorer.flush()?;
            Ok(head + &tail)
        });
        match restored {
            Ok(out) => Ok(Bytes::from(out)),
            Err(_) => Ok(Bytes::from(
                "event: error\ndata: {\"error\":{\"type\":\"dgw_restore_failed\"}}\n\n",
            )),
        }
    } else if content_type.contains("json") || looks_like_json(req_headers, bytes) {
        match restore_json_body(state.store.as_ref(), &creator, bytes) {
            Ok(out) => Ok(out.into()),
            Err(_) => Err((StatusCode::BAD_GATEWAY, "restore failed").into_response()),
        }
    } else {
        Ok(bytes.clone())
    }
}
