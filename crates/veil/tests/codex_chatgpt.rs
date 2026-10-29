use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    response::Response,
    Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use veil::{
    admin::TrafficLog,
    proxy::{router, AppState, UpstreamConfig},
};
use veil_engine::mapping::MemoryStore;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<(String, HeaderMap, Value)>>>);

async fn upstream(State(cap): State<Capture>, req: Request<Body>) -> Response {
    let path = req.uri().to_string();
    let headers = req.headers().clone();
    let raw = req.into_body().collect().await.unwrap().to_bytes();
    let body: Value = if raw.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&raw).unwrap()
    };
    cap.0
        .lock()
        .unwrap()
        .push((path.clone(), headers, body.clone()));
    if path.starts_with("/backend-api/codex/models") {
        return Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"models":[]}"#))
            .unwrap();
    }
    let text = body["input"][0]["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("13800138000"));
    assert!(text.contains("{{PHONE_"));
    let at = text.find("{{PHONE_").unwrap();
    let end = at + text[at..].find("}}").unwrap() + 2;
    let token = &text[at..end];
    let mid = token.len() / 2;
    let events = [
        json!({"type":"response.output_text.delta", "delta":format!("已收到 {}", &token[..mid])}),
        json!({"type":"response.output_text.delta", "delta":format!("{}。", &token[mid..])}),
        json!({"type":"response.completed", "response":{"status":"completed", "output":[{"content":[{"text":format!("已收到 {token}。")}]}]}}),
    ];
    let chunks = events.into_iter().map(|v| {
        Ok::<_, std::io::Error>(Bytes::from(format!(
            "event: {}\ndata: {v}\n\n",
            v["type"].as_str().unwrap()
        )))
    });
    // Real subscription responses may have no Content-Type at all.
    Response::builder()
        .body(Body::from_stream(futures_util::stream::iter(chunks)))
        .unwrap()
}

async fn bind(app: Router) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (port, task)
}

#[tokio::test]
async fn subscription_routes_preserve_auth_redact_and_restore_stream() {
    let capture = Capture::default();
    let (up_port, up_task) =
        bind(Router::new().fallback(upstream).with_state(capture.clone())).await;
    let mut state = AppState::new(
        Arc::new(MemoryStore::new()),
        UpstreamConfig {
            anthropic_upstream: "http://127.0.0.1:1".into(),
            openai_completions_upstream: "http://127.0.0.1:1".into(),
            openai_responses_upstream: "http://127.0.0.1:1".into(),
        },
        32 * 1024 * 1024,
    );
    state.codex_chatgpt_upstream = format!("http://127.0.0.1:{up_port}/backend-api/codex");
    let traffic = TrafficLog::default();
    let (port, proxy_task) = bind(router(state.with_traffic(traffic.clone()))).await;
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let models = client
        .get(format!(
            "http://127.0.0.1:{port}/codex/models?client_version=test"
        ))
        .header(header::AUTHORIZATION, "Bearer test-oauth")
        .header("chatgpt-account-id", "test-account")
        .send()
        .await
        .unwrap();
    assert_eq!(models.status(), StatusCode::OK);
    let response = client.post(format!("http://127.0.0.1:{port}/codex/responses"))
        .header(header::AUTHORIZATION, "Bearer test-oauth")
        .header("chatgpt-account-id", "test-account")
        .json(&json!({"stream":true,"input":[{"role":"user","content":[{"type":"input_text","text":"请重复 13800138000"}]}]}))
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let mut stream = response.bytes_stream();
    let mut out = String::new();
    while let Some(chunk) = stream.next().await {
        out.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    let mut text = String::new();
    let mut completed = false;
    for line in out.lines().filter_map(|l| l.strip_prefix("data: ")) {
        let v: Value = serde_json::from_str(line).unwrap();
        if v["type"] == "response.output_text.delta" {
            text.push_str(v["delta"].as_str().unwrap());
        }
        if v["type"] == "response.completed" {
            completed = true;
            assert_eq!(
                v["response"]["output"][0]["content"][0]["text"],
                "已收到 13800138000。"
            );
        }
    }
    assert_eq!(text, "已收到 13800138000。");
    assert!(completed);
    assert!(!out.contains("{{PHONE_"));
    let hits = capture.0.lock().unwrap();
    assert_eq!(hits[0].0, "/backend-api/codex/models?client_version=test");
    assert_eq!(hits[1].0, "/backend-api/codex/responses");
    for (_, headers, _) in hits.iter() {
        assert_eq!(headers[header::AUTHORIZATION], "Bearer test-oauth");
        assert_eq!(headers["chatgpt-account-id"], "test-account");
    }
    assert!(traffic
        .snapshot()
        .iter()
        .any(|e| e.streaming && e.hit_types.contains(&"PHONE".into())));
    proxy_task.abort();
    up_task.abort();
}
