//! End-to-end desensitize + restore through the proxy against a mock upstream.
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::response::IntoResponse;
use axum::Router;
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use veil::proxy::{router, AppState, UpstreamConfig};
use veil_engine::mapping::MemoryStore;

#[derive(Clone, Default)]
struct Cap {
    body: Arc<Mutex<Option<Bytes>>>,
}

async fn echo(State(cap): State<Cap>, req: Request<Body>) -> impl IntoResponse {
    let body = req.into_body().collect().await.unwrap().to_bytes();
    *cap.body.lock().unwrap() = Some(body.clone());
    let echoed = String::from_utf8_lossy(&body).into_owned();
    (
        [(header::CONTENT_TYPE, "application/json")],
        json!({ "content": echoed }).to_string(),
    )
}

#[tokio::test]
async fn messages_roundtrip_restores_phone() {
    let cap = Cap::default();
    let mock = Router::new().fallback(echo).with_state(cap.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });

    let origin = format!("http://127.0.0.1:{up}");
    let store = Arc::new(MemoryStore::new());
    let state = AppState::new(
        store,
        UpstreamConfig {
            anthropic_upstream: origin.clone(),
            openai_completions_upstream: origin.clone(),
            openai_responses_upstream: origin,
        },
        32 * 1024 * 1024,
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, router(state)).await.unwrap();
    });

    let payload = json!({"messages": [{"content": "call 13800138000"}]});
    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/messages"))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let upstream = cap.body.lock().unwrap().clone().unwrap();
    let up_text = String::from_utf8_lossy(&upstream);
    assert!(!up_text.contains("13800138000"), "{up_text}");
    assert!(up_text.contains("{{PHONE_"));
    let body: Value = resp.json().await.unwrap();
    let content = body["content"].as_str().unwrap();
    assert!(content.contains("13800138000"), "{content}");
    assert!(!content.contains("{{PHONE_"));
}
