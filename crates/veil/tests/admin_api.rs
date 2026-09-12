use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use veil::admin::{router, AdminState, TrafficLog};
use veil::config::Config;

async fn bind_admin(data_dir: std::path::PathBuf, cfg: Config, loopback: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = AdminState {
        data_dir,
        config: Arc::new(Mutex::new(cfg)),
        loopback,
        traffic: TrafficLog::default(),
        proxy_port: 18787,
        management_port: port,
    };
    let app = router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

fn loaded(dir: &std::path::Path) -> (Config, String) {
    let cfg = Config::load(dir).unwrap();
    let token = std::fs::read_to_string(dir.join("admin.token")).unwrap();
    (cfg, token.trim().to_string())
}

#[tokio::test]
async fn status_ok_on_loopback_without_token() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, _token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let url = format!("http://127.0.0.1:{port}/api/status");
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Veil"));
    assert!(!body.contains("new_key"));
    assert!(body.contains("master_key_set"));
    assert!(!body.to_ascii_lowercase().contains("13800138000"));
}

#[tokio::test]
async fn mutating_without_token_is_401() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, _token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let url = format!("http://127.0.0.1:{port}/api/rules");
    let resp = reqwest::Client::new()
        .put(&url)
        .json(&Vec::<serde_json::Value>::new())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn put_rules_with_token() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let client = reqwest::Client::new();
    let rules_url = format!("http://127.0.0.1:{port}/api/rules");
    let resp = client
        .put(&rules_url)
        .header("x-veil-admin-token", &token)
        .json(&serde_json::json!([{
            "id": "phone",
            "type_prefix": "PHONE",
            "enabled": true,
            "priority": 10,
            "pattern": "1[3-9]\\d{9}",
            "source": "custom"
        }]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn master_key_rotate_does_not_echo() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let new_key = "ab".repeat(32);
    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/api/master-key"))
        .header("x-veil-admin-token", &token)
        .json(&serde_json::json!({"new_key": new_key}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(!body.contains(&new_key));
    assert!(body.contains("rotated"));
}

#[tokio::test]
async fn traffic_sse_has_no_body_field() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let resp = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/api/traffic"))
        .header("x-veil-admin-token", &token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("event: snapshot"));
    assert!(!body.contains("\"body\""));
    assert!(!body.contains("plaintext"));
}

#[tokio::test]
async fn non_loopback_requires_token_even_for_status() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, _token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, false).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/status"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[allow(dead_code)]
fn hash_token(t: &str) -> String {
    hex::encode(Sha256::digest(t.as_bytes()))
}

#[tokio::test]
async fn clients_lists_all_adapters() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg, _token) = loaded(dir.path());
    let port = bind_admin(dir.path().to_path_buf(), cfg, true).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/clients"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Vec<serde_json::Value> = resp.json().await.unwrap();
    let ids: Vec<_> = list.iter().map(|c| c["id"].as_str().unwrap().to_string()).collect();
    assert!(ids.contains(&"claude".into()));
    assert!(ids.contains(&"codex".into()));
    assert!(ids.contains(&"trae".into()));
    assert!(ids.contains(&"pi".into()));
    assert!(ids.contains(&"hermes".into()));
    assert!(ids.contains(&"grok".into()));
    assert!(ids.contains(&"codebuddy".into()));
    assert_eq!(list.len(), 7);
}
