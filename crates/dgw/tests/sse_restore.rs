use dgw::proxy::{restore_json_body, SseRestorer};
use dgw_engine::creator::anonymous_creator;
use dgw_engine::mapping::{DownStore, MappingStore, MemoryStore};
use dgw_engine::placeholder::Placeholder;
use serde_json::{json, Value};

fn phone_token(store: &MemoryStore) -> String {
    store
        .get_or_insert(&anonymous_creator(), "PHONE", "13800138000")
        .unwrap()
        .format()
}

fn event_with_text(event: &str, text: &str) -> String {
    let payload = json!({
        "type": "content_block_delta",
        "delta": {"type": "text_delta", "text": text}
    });
    let mut s = String::new();
    if !event.is_empty() {
        s.push_str("event: ");
        s.push_str(event);
        s.push('\n');
    }
    s.push_str("data: ");
    s.push_str(&payload.to_string());
    s.push_str("\n\n");
    s
}

fn collect_text(sse: &str, pointer: &str) -> String {
    let mut out = String::new();
    for block in sse.split("\n\n") {
        for line in block.lines() {
            let Some(data) = line.strip_prefix("data:") else { continue; };
            let Ok(v) = serde_json::from_str::<Value>(data.trim()) else { continue; };
            if let Some(s) = v.pointer(pointer).and_then(|x| x.as_str()) {
                out.push_str(s);
            }
        }
    }
    out
}

#[test]
fn anthropic_placeholder_split_across_events() {
    let store = MemoryStore::new();
    let token = phone_token(&store);
    let mid = token.len() / 2;
    let mut first = String::from("call ");
    first.push_str(&token[..mid]);
    let mut second = String::from(&token[mid..]);
    second.push_str(" now");
    let mut r = SseRestorer::new(&store, anonymous_creator());
    let mut out = r.push(&event_with_text("content_block_delta", &first)).unwrap();
    out.push_str(&r.push(&event_with_text("content_block_delta", &second)).unwrap());
    out.push_str(&r.flush().unwrap());
    assert!(out.contains("event: content_block_delta"));
    assert_eq!(collect_text(&out, "/delta/text"), "call 13800138000 now");
}

#[test]
fn openai_chat_split_across_events() {
    let store = MemoryStore::new();
    let token = store
        .get_or_insert(&anonymous_creator(), "IP", "8.8.8.8")
        .unwrap()
        .format();
    let mid = token.len() / 2;
    let e1 = json!({"choices": [{"delta": {"content": &token[..mid]}}]});
    let e2 = json!({"choices": [{"delta": {"content": &token[mid..]}}]});
    let mut r = SseRestorer::new(&store, anonymous_creator());
    let mut s1 = String::from("data: ");
    s1.push_str(&e1.to_string());
    s1.push_str("\n\n");
    let mut s2 = String::from("data: ");
    s2.push_str(&e2.to_string());
    s2.push_str("\n\n");
    let mut out = r.push(&s1).unwrap();
    out.push_str(&r.push(&s2).unwrap());
    out.push_str(&r.flush().unwrap());
    assert_eq!(collect_text(&out, "/choices/0/delta/content"), "8.8.8.8");
}

#[test]
fn openai_responses_delta_string() {
    let store = MemoryStore::new();
    let token = phone_token(&store);
    let mid = token.len() / 2;
    let e1 = json!({"delta": &token[..mid]});
    let e2 = json!({"delta": &token[mid..]});
    let mut r = SseRestorer::new(&store, anonymous_creator());
    let mut s1 = String::from("event: response.output_text.delta\ndata: ");
    s1.push_str(&e1.to_string());
    s1.push_str("\n\n");
    let mut s2 = String::from("event: response.output_text.delta\ndata: ");
    s2.push_str(&e2.to_string());
    s2.push_str("\n\n");
    let mut out = r.push(&s1).unwrap();
    out.push_str(&r.push(&s2).unwrap());
    out.push_str(&r.flush().unwrap());
    assert!(out.contains("event: response.output_text.delta"));
    assert_eq!(collect_text(&out, "/delta"), "13800138000");
}

#[test]
fn non_stream_json_restores_content() {
    let store = MemoryStore::new();
    let token = phone_token(&store);
    let mut content = String::from("hi ");
    content.push_str(&token);
    let body = serde_json::to_vec(&json!({"content": content})).unwrap();
    let out = restore_json_body(&store, &anonymous_creator(), &body).unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["content"], "hi 13800138000");
}

#[test]
fn down_store_with_valid_token_errors() {
    let fake = Placeholder::fresh("PHONE").format();
    let ev = event_with_text("content_block_delta", &fake);
    let mut r = SseRestorer::new(&DownStore, anonymous_creator());
    let err = r.push(&ev).unwrap_err();
    assert!(matches!(err, dgw_engine::walk::WalkError::StoreUnavailable));
}
