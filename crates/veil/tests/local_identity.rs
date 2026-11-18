use axum::http::StatusCode;
use std::sync::{Arc, Mutex};
use veil::admin::{router, AdminState, TrafficLog};
use veil::config::{Config, Mode, RuleConfig, RuleSource};
use veil::local_identity::{generate, Collected, IdentityConfig, Selection};
use veil::ruleset::ruleset_from_config;

fn selected() -> Selection {
    Selection {
        enabled: true,
        app_ids: true,
        hardware: true,
        user_context: true,
        git_email: true,
    }
}
fn fixture() -> Collected {
    Collected {
        device_ids: vec!["a1".repeat(32)],
        machine_ids: vec!["b2".repeat(32)],
        installs: vec!["12345678-1234-4321-abcd-9876543210ab".into()],
        machine_guid: vec!["22345678-1234-4321-abcd-9876543210ab".into()],
        host: "DESKTOP-FIXTURE".into(),
        user: "Fixture.User+".into(),
        sid: "S-1-5-21-111111111-222222222-333333333-1001".into(),
        smbios: vec!["32345678-1234-4321-abcd-9876543210ab".into()],
        sqm: vec!["{42345678-1234-4321-abcd-9876543210ab}".into()],
        product: vec!["11111-22222-33333-ABCDE".into()],
        disks: vec!["FAKE_DISK-1234567.".into()],
        volumes: vec!["A1B2C3D4".into()],
        macs: vec!["AA-BB-CC-DD-EE-12".into()],
        serials: vec!["FAKEBOARD5678".into()],
        emails: vec!["fixture+git@example.invalid".into()],
    }
}
fn identity() -> IdentityConfig {
    let mut id = IdentityConfig::default();
    id.selection = selected();
    id.merge(generate(&fixture(), &id.selection)).unwrap();
    id
}

#[test]
fn generates_all_fifteen_without_replacing_builtins_or_custom_rules() {
    let mut cfg = Config::default();
    let original = ruleset_from_config(&cfg).rules().len();
    cfg.local_identity = identity();
    assert_eq!(cfg.local_identity.rules.len(), 15);
    assert_eq!(ruleset_from_config(&cfg).rules().len(), original + 15);
    cfg.rules = vec![RuleConfig {
        id: "my-rule".into(),
        type_prefix: "MYRULE".into(),
        enabled: true,
        priority: 50,
        pattern: Some("MY_CUSTOM_LITERAL".into()),
        words: None,
        source: RuleSource::Custom,
    }];
    let rules = ruleset_from_config(&cfg);
    assert_eq!(rules.rules().len(), 16);
    assert!(rules
        .find_hits("MY_CUSTOM_LITERAL")
        .iter()
        .any(|h| h.type_prefix == "MYRULE"));
}

#[test]
fn exact_values_variants_and_boundaries() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut cfg = Config::default();
    cfg.local_identity = identity();
    let rules = ruleset_from_config(&cfg);
    let cases = vec![
        ("a1".repeat(32), "DEVICEID"),
        ("b2".repeat(6), "MACHINEID"),
        (
            STANDARD.encode(fixture().installs[0].as_bytes()),
            "INSTALLID",
        ),
        ("2234567812344321abcd9876543210ab".into(), "WINID"),
        (r"C:\Users\Fixture.User+\.claude".into(), "USERNAME"),
        (r"C:\\Users\\Fixture.User+\\x".into(), "USERNAME"),
        ("DESKTOP-FIXTURE".into(), "HOSTNAME"),
        (
            "S-1-5-21-111111111-222222222-333333333-500".into(),
            "USERSID",
        ),
        ("{32345678-1234-4321-abcd-9876543210ab}".into(), "HWUUID"),
        ("4234567812344321abcd9876543210ab".into(), "SQMID"),
        ("11111-22222-33333-ABCDE".into(), "PRODUCTID"),
        ("FAKEDISK1234567".into(), "DISKSN"),
        ("A1B2-C3D4".into(), "VOLSN"),
        ("aa:bb:cc:dd:ee:12".into(), "MAC"),
        ("FAKEBOARD5678".into(), "BIOSSN"),
        ("<FIXTURE+GIT@EXAMPLE.INVALID>".into(), "GITEMAIL"),
    ];
    for (text, prefix) in cases {
        assert!(
            rules
                .find_hits(&text)
                .iter()
                .any(|h| h.type_prefix == prefix),
            "missing type {prefix}"
        );
    }
    let user = rules.find_hits("before\nFixture.User+\r\nafter");
    assert!(user
        .iter()
        .any(|h| h.type_prefix == "USERNAME" && h.plaintext == "Fixture.User+"));
    for text in [
        "OTHER-DESKTOP",
        r"C:\Users\FixtureX\x",
        r"C:\Users\Fixture.User+.smith\x",
        r"C:\Users\Fixture.User+中文\x",
        "USER=Fixture.User+.smith",
        "Fixture.User+ title",
        "FixtureXUser+",
        "fixture-image",
        "33345678-1234-4321-abcd-9876543210ab",
        "aa:bb:cc:dd:ee:13",
    ] {
        assert!(!rules
            .find_hits(text)
            .iter()
            .any(|h| h.type_prefix == "USERNAME"
                || h.type_prefix == "HOSTNAME"
                || h.type_prefix == "HWUUID"
                || h.type_prefix == "MAC"));
    }
    assert!(!rules
        .find_hits(&format!("{}c", "a1".repeat(32)))
        .iter()
        .any(|h| h.type_prefix == "DEVICEID"));
}

#[test]
fn refresh_preserves_old_values_and_does_not_duplicate_identical_scans() {
    let mut id = identity();
    let original = serde_json::to_value(&id).unwrap();
    id.merge(generate(&fixture(), &selected())).unwrap();
    assert_eq!(
        serde_json::to_value(&id).unwrap()["rules"],
        original["rules"]
    );
    let mut other = fixture();
    other.host = "SECOND-FIXTURE".into();
    other.macs.clear();
    id.merge(generate(&other, &selected())).unwrap();
    let mut cfg = Config::default();
    cfg.local_identity = id;
    let rules = ruleset_from_config(&cfg);
    for host in ["DESKTOP-FIXTURE", "SECOND-FIXTURE"] {
        assert!(rules
            .find_hits(host)
            .iter()
            .any(|h| h.type_prefix == "HOSTNAME"));
    }
    assert!(rules
        .find_hits("AA-BB-CC-DD-EE-12")
        .iter()
        .any(|h| h.type_prefix == "MAC"));
}

#[test]
fn selection_disable_and_roundtrip_do_not_lose_identifiers() {
    let mut cfg = Config::default();
    cfg.local_identity = identity();
    let raw = toml::to_string(&cfg).unwrap();
    let recovered: Config = toml::from_str(&raw).unwrap();
    assert_eq!(cfg, recovered);
    cfg.local_identity.selection.hardware = false;
    assert!(!ruleset_from_config(&cfg)
        .find_hits("AA-BB-CC-DD-EE-12")
        .iter()
        .any(|h| h.type_prefix == "MAC"));
    cfg.local_identity.selection.enabled = false;
    assert!(!ruleset_from_config(&cfg)
        .find_hits("DESKTOP-FIXTURE")
        .iter()
        .any(|h| h.type_prefix == "HOSTNAME"));
    assert_eq!(cfg.local_identity.rules.len(), 15);
    assert!(!format!("{:?}", cfg.local_identity).contains("DESKTOP-FIXTURE"));
}

#[test]
fn running_snapshot_distinguishes_saved_settings_and_ignores_scan_timestamp() {
    let running = identity();
    let mut saved = running.clone();
    saved.last_scan = Some(1);
    let unchanged = saved.view(&running);
    assert!(unchanged.running_enabled);
    assert_eq!(unchanged.running_rule_count, 15);
    assert!(!unchanged.restart_required);
    saved.selection.enabled = false;
    let stopping = saved.view(&running);
    assert!(stopping.running_enabled);
    assert!(!stopping.selection.enabled);
    assert!(stopping.restart_required);
    let restarted = saved.view(&saved);
    assert!(!restarted.running_enabled);
    assert_eq!(restarted.running_rule_count, 0);
    assert!(!restarted.restart_required);
    let starting = running.view(&saved);
    assert!(!starting.running_enabled);
    assert!(starting.selection.enabled);
    assert!(starting.restart_required);
}

#[test]
fn generic_or_invalid_values_are_not_turned_into_rules() {
    let mut v = Collected::default();
    v.serials = vec![
        "To Be Filled By O.E.M.".into(),
        "Default string".into(),
        "000000".into(),
    ];
    v.smbios = vec![
        "00000000-0000-0000-0000-000000000000".into(),
        "x-1234".into(),
    ];
    v.device_ids = vec!["short".into()];
    v.sid = "S-1-5-32-544".into();
    v.macs = vec!["00-00-00-00-00-00".into()];
    assert!(generate(&v, &selected()).is_empty());
}

async fn server(cfg: Config) -> (tempfile::TempDir, u16, String) {
    let dir = tempfile::tempdir().unwrap();
    cfg.save(dir.path()).unwrap();
    let cfg = Config::load(dir.path()).unwrap();
    let token = std::fs::read_to_string(dir.path().join("admin.token")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = AdminState {
        data_dir: dir.path().into(),
        config: Arc::new(Mutex::new(cfg)),
        loopback: true,
        traffic: TrafficLog::default(),
        proxy_port: 18787,
        management_port: port,
    };
    tokio::spawn(async move {
        axum::serve(listener, router(state)).await.unwrap();
    });
    (dir, port, token.trim().into())
}

#[tokio::test]
async fn api_requires_authentication_and_returns_only_counts_not_patterns() {
    let mut cfg = Config::default();
    cfg.local_identity = identity();
    let (_dir, port, token) = server(cfg).await;
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/api/local-identity");
    assert_eq!(
        client.get(&url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(format!("{url}/refresh"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .put(&url)
            .json(&selected())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let body = client
        .get(&url)
        .header("x-veil-admin-token", &token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["rules"].as_array().unwrap().len(), 15);
    assert_eq!(json["running_enabled"], true);
    assert_eq!(json["running_rule_count"], 15);
    assert_eq!(json["restart_required"], false);
    for secret in [
        "DESKTOP-FIXTURE",
        "Fixture.User+",
        "FAKEBOARD5678",
        "example.invalid",
        "patterns",
    ] {
        assert!(!body.contains(secret));
    }
    let rule_text = client
        .get(format!("http://127.0.0.1:{port}/api/rules"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!rule_text.contains("DESKTOP-FIXTURE"));
    assert!(!rule_text.contains("local-identity/"));
    let mut off = selected();
    off.enabled = false;
    let response = client
        .put(&url)
        .header("x-veil-admin-token", &token)
        .json(&off)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["enabled"], false);
    assert_eq!(updated["running_enabled"], true);
    assert_eq!(updated["running_rule_count"], 15);
    assert_eq!(updated["restart_required"], true);
    assert_eq!(updated["rules"].as_array().unwrap().len(), 15);
}

#[tokio::test]
async fn server_mode_cannot_collect_on_remote_server() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.mode = Mode::Server;
    cfg.admin_token_hash = hex::encode(sha2::Sha256::digest(b"test-server-admin"));
    use sha2::Digest;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = AdminState {
        data_dir: dir.path().into(),
        config: Arc::new(Mutex::new(cfg)),
        loopback: true,
        traffic: TrafficLog::default(),
        proxy_port: 18787,
        management_port: port,
    };
    tokio::spawn(async move {
        axum::serve(listener, router(state)).await.unwrap();
    });
    let response = reqwest::Client::new()
        .put(format!("http://127.0.0.1:{port}/api/local-identity"))
        .header("x-veil-admin-token", "test-server-admin")
        .json(&selected())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn saved_patterns_are_caught_by_their_own_rules() {
    let mut cfg = Config::default();
    cfg.local_identity = identity();
    // Same text a tool would read from config.toml and send upstream.
    let text = toml::to_string(&cfg.local_identity).unwrap();
    assert!(!text.contains(r"\-"), "needless escapes leak values");
    assert!(
        !text.contains("[-:]?"),
        "MAC must be listed as literal forms"
    );
    let hits = ruleset_from_config(&cfg).find_hits(&text);
    for value in [
        "DESKTOP-FIXTURE",
        "11111-22222-33333-ABCDE",
        "S-1-5-21-111111111-222222222-333333333",
        "22345678-1234-4321-abcd-9876543210ab",
        "32345678-1234-4321-abcd-9876543210ab",
        "42345678-1234-4321-abcd-9876543210ab",
        "12345678-1234-4321-abcd-9876543210ab",
        "AA-BB-CC-DD-EE-12",
        "AA:BB:CC:DD:EE:12",
        "AABBCCDDEE12",
        "A1B2-C3D4",
        "FAKEBOARD5678",
        "FAKE_DISK-1234567",
    ] {
        assert!(text.contains(value), "{value} is not literal in saved text");
        assert!(
            hits.iter().any(|h| h.plaintext.eq_ignore_ascii_case(value)),
            "{value} in saved text is not caught"
        );
    }
}

#[test]
fn refresh_rewrites_legacy_escaped_patterns_without_duplicates() {
    let fresh = identity();
    let mut legacy = fresh.clone();
    for r in &mut legacy.rules {
        match r.id.as_str() {
            "hostname" => {
                r.patterns =
                    vec![r"(?i)(?<![A-Za-z0-9_-])(?:DESKTOP\-FIXTURE)(?![A-Za-z0-9_-])".into()]
            }
            "mac-address" => {
                r.patterns = vec![
                    r"(?i)(?<![0-9a-f])(?:AA[-:]?BB[-:]?CC[-:]?DD[-:]?EE[-:]?12)(?![0-9a-f])"
                        .into(),
                ]
            }
            "git-email" => {
                r.patterns = vec![
                    r"(?i)(?<![A-Za-z0-9_.+-])(?:fixture\+git@example\.invalid)(?![A-Za-z0-9_.+-])"
                        .into(),
                ]
            }
            _ => {}
        }
    }
    assert!(legacy.rules != fresh.rules);
    legacy.merge(generate(&fixture(), &selected())).unwrap();
    assert!(
        legacy.rules == fresh.rules,
        "legacy patterns were not rewritten in place"
    );
}
