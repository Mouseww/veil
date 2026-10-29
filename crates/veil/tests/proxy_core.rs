use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::IntoResponse;

use axum::Router;
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use veil::proxy::{router, AppState, UpstreamConfig};
use veil_engine::creator::anonymous_creator;
use veil_engine::mapping::{DownStore, MappingStore, MemoryStore};

#[derive(Clone, Default)]
struct Capture {
    hits: Arc<Mutex<Vec<Captured>>>,
}

#[derive(Clone, Debug)]
struct Captured {
    method: String,
    path: String,
    headers: HeaderMap,
    body: Bytes,
}

async fn capture_handler(State(cap): State<Capture>, req: Request<Body>) -> impl IntoResponse {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let headers = req.headers().clone();
    let body = req.into_body().collect().await.unwrap().to_bytes();
    cap.hits.lock().unwrap().push(Captured {
        method,
        path,
        headers,
        body,
    });
    (StatusCode::OK, "{\"ok\":true}")
}

async fn bind(router: Router) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (addr.port(), handle)
}

async fn setup(
    store: Arc<dyn MappingStore + Send + Sync>,
    limit: usize,
) -> (u16, Capture, Arc<dyn MappingStore + Send + Sync>) {
    let cap = Capture::default();
    let mock = Router::new()
        .fallback(capture_handler)
        .with_state(cap.clone());
    let (up_port, _) = bind(mock).await;
    let origin = format!("http://127.0.0.1:{up_port}");
    let state = AppState::new(
        store.clone(),
        UpstreamConfig {
            anthropic_upstream: origin.clone(),
            openai_completions_upstream: origin.clone(),
            openai_responses_upstream: origin,
        },
        limit,
    );
    let (proxy_port, _) = bind(router(state)).await;
    (proxy_port, cap, store)
}

#[tokio::test]
async fn proxy_reuses_chat_and_codex_items_and_keeps_top_level_redaction() {
    let cap = Capture::default();
    let (up_port, _) = bind(
        Router::new()
            .fallback(capture_handler)
            .with_state(cap.clone()),
    )
    .await;
    let origin = format!("http://127.0.0.1:{up_port}");
    let mut state = AppState::new(
        Arc::new(MemoryStore::new()),
        UpstreamConfig {
            anthropic_upstream: origin.clone(),
            openai_completions_upstream: origin.clone(),
            openai_responses_upstream: origin.clone(),
        },
        1024 * 1024,
    );
    state.codex_chatgpt_upstream = origin;
    state.alias_hint = true;
    let cache = state.message_cache.clone();
    assert!(Arc::ptr_eq(&cache, &state.clone().message_cache));
    let (port, _) = bind(router(state)).await;
    for (path, field) in [
        ("/v1/chat/completions", "messages"),
        ("/codex/responses", "input"),
    ] {
        let body = json!({field:[{"role":"user","content":"13800138000"}], "instructions":"server 10.0.0.5"});
        for _ in 0..2 {
            let (status, _, _) = send(
                port,
                "POST",
                path,
                Some("application/json"),
                serde_json::to_vec(&body).unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
    }
    assert_eq!(cache.stats().hits, 2);
    let captured = cap.hits.lock().unwrap();
    assert_eq!(captured.len(), 4);
    assert_eq!(captured[0].body, captured[1].body);
    assert_eq!(captured[2].body, captured[3].body);
    for request in captured.iter() {
        let body = std::str::from_utf8(&request.body).unwrap();
        assert!(!body.contains("13800138000") && !body.contains("10.0.0.5"));
        assert!(body.contains("{{PHONE_") && body.contains("{{IP_"));
    }
}

async fn send(
    port: u16,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Bytes, HeaderMap) {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}{path}");
    let mut b = client.request(method.parse().unwrap(), url);
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
    }
    b = b.header(header::ACCEPT_ENCODING, "gzip, br");
    if !body.is_empty() {
        b = b.body(body);
    }
    let resp = b.send().await.unwrap();
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap();
    let headers = resp.headers().clone();
    let bytes = resp.bytes().await.unwrap();
    (status, bytes, headers)
}

#[tokio::test]
async fn get_models_empty_is_forwarded() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let (status, _, _) = send(port, "GET", "/v1/models", None, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    let hits = cap.hits.lock().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "/v1/models");
    assert_eq!(hits[0].method, "GET");
}

#[tokio::test]
async fn json_post_without_hits_is_unchanged() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let body = serde_json::to_vec(&json!({"messages":[{"content":"hello"}]})).unwrap();
    let (status, _, _) = send(
        port,
        "POST",
        "/v1/messages",
        Some("application/json"),
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hits = cap.hits.lock().unwrap();
    let got: Value = serde_json::from_slice(&hits[0].body).unwrap();
    let want: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(got, want);
}

#[tokio::test]
async fn phone_is_redacted_before_upstream() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, store) = setup(store, 32 * 1024 * 1024).await;
    let body = serde_json::to_vec(&json!({"messages":[{"content":"call 13800138000"}]})).unwrap();
    let (status, _, _) = send(port, "POST", "/v1/messages", Some("application/json"), body).await;
    assert_eq!(status, StatusCode::OK);
    let hits = cap.hits.lock().unwrap();
    let text = String::from_utf8_lossy(&hits[0].body);
    assert!(!text.contains("13800138000"), "{text}");
    assert!(text.contains("{{PHONE_"), "{text}");
    let mapped = store
        .get_or_insert(&anonymous_creator(), "PHONE", "13800138000")
        .unwrap();
    assert!(text.contains(&mapped.format()));
}

#[tokio::test]
async fn oversize_body_is_413_and_not_forwarded() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 8).await;
    let body = serde_json::to_vec(&json!({"messages":[{"content":"hello world"}]})).unwrap();
    assert!(body.len() > 8);
    let (status, _, _) = send(port, "POST", "/v1/messages", Some("application/json"), body).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(cap.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn accept_encoding_identity_to_upstream() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let _ = send(port, "GET", "/v1/models", None, vec![]).await;
    let hits = cap.hits.lock().unwrap();
    let ae = hits[0]
        .headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(ae, "identity");
    assert!(!ae.to_ascii_lowercase().contains("gzip"));
}

#[tokio::test]
async fn multipart_is_415() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let (status, _, _) = send(
        port,
        "POST",
        "/v1/messages",
        Some("multipart/form-data; boundary=x"),
        b"--x--".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(cap.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unknown_path_is_404() {
    let store = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let (status, _, _) = send(port, "GET", "/secret", None, vec![]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(cap.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn down_store_with_phone_is_502() {
    let store = Arc::new(DownStore);
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let body = serde_json::to_vec(&json!({"messages":[{"content":"call 13800138000"}]})).unwrap();
    let (status, _, _) = send(port, "POST", "/v1/messages", Some("application/json"), body).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(cap.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn down_store_get_models_still_forwards() {
    let store = Arc::new(DownStore);
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let (status, _, _) = send(port, "GET", "/v1/models", None, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cap.hits.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn base64_image_body_not_corrupted() {
    let store: Arc<dyn MappingStore + Send + Sync> = Arc::new(MemoryStore::new());
    let (port, cap, _) = setup(store, 32 * 1024 * 1024).await;
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let body = serde_json::json!({
        "model": "claude",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "phone 13800138000"},
                {"type": "image", "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": png
                }}
            ]
        }]
    });
    let (status, _, _) = send(
        port,
        "POST",
        "/v1/messages",
        Some("application/json"),
        serde_json::to_vec(&body).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hits = cap.hits.lock().unwrap();
    assert_eq!(hits.len(), 1);
    let forwarded: Value = serde_json::from_slice(&hits[0].body).unwrap();
    let text = forwarded["messages"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("{{PHONE_"), "{text}");
    assert_eq!(
        forwarded["messages"][0]["content"][1]["source"]["data"]
            .as_str()
            .unwrap(),
        png
    );
}
