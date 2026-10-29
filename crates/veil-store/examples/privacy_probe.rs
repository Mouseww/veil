//! Local evaluation adapter. JSONL stdin/stdout; never contacts an upstream.
use serde_json::{json, Value};
use std::io::{self, BufRead};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use veil_engine::builtin::builtin_ruleset;
use veil_engine::creator::anonymous_creator;
use veil_engine::mapping::{MappingStore, MemoryStore};
use veil_engine::rules::{Allowlist, RuleSet};
use veil_engine::walk::{desensitize_json, restore_json};
use veil_store::SqliteStore;

fn rules(v: &Value) -> RuleSet {
    let base = builtin_ruleset();
    let mut rs = base.rules().to_vec();
    for r in &mut rs {
        if v["email"].as_bool() == Some(true) && r.id == "email" {
            r.enabled = true;
        }
        if let Some(only) = v["only"].as_str() {
            r.enabled &= r.id == only;
        }
        if v["omit"].as_str() == Some(r.id.as_str()) {
            r.enabled = false;
        }
    }
    RuleSet::new(
        rs,
        Allowlist::from(["127.0.0.1", "localhost", "0.0.0.0", "::1"]),
    )
}

fn pipeline(text: &str, rs: &RuleSet, store: &dyn MappingStore, roundtrip: bool) -> Value {
    // Include request parse and re-encode, matching the local JSON body path.
    let wire = serde_json::to_vec(&json!({"messages":[{"role":"user","content":text}]})).unwrap();
    let input: Value = serde_json::from_slice(&wire).unwrap();
    let out = desensitize_json(&input, rs, store, &anonymous_creator()).unwrap();
    std::hint::black_box(serde_json::to_vec(&out).unwrap());
    if roundtrip {
        assert_eq!(
            restore_json(&out, store, &anonymous_creator()).unwrap(),
            input
        );
    }
    out
}

fn run(v: Value) -> Value {
    if v["op"] == "rules" {
        return json!(builtin_ruleset().rules().iter().map(|r| json!({"id":r.id,"type":r.type_prefix,"enabled":r.enabled,"priority":r.priority,"pattern":r.pattern,"timeout_ms":r.timeout_ms})).collect::<Vec<_>>());
    }
    let rs = rules(&v);
    if v["op"] == "predict" {
        let text = v["text"].as_str().unwrap();
        let now = Instant::now();
        let hits = rs.find_hits(text);
        let ns = now.elapsed().as_nanos();
        let store = MemoryStore::new();
        let out = pipeline(text, &rs, &store, true);
        let masked = out["messages"][0]["content"].as_str().unwrap();
        let spans: Vec<_> = hits.iter().map(|h| json!({"type":h.type_prefix,"start":text[..h.start].chars().count(),"end":text[..h.end].chars().count(),"byte_start":h.start,"byte_end":h.end})).collect();
        return json!({"hits":spans,"detect_ns":ns,"roundtrip_ok":true,"masked":masked});
    }
    let texts: Arc<Vec<String>> = Arc::new(
        v["texts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect(),
    );
    assert!(!texts.is_empty());
    let mode = v["mode"].as_str().unwrap_or("detect").to_owned();
    let threads = v["threads"].as_u64().unwrap_or(1) as usize;
    let duration = Duration::from_millis(v["duration_ms"].as_u64().unwrap_or(500));
    let temp = tempfile::tempdir().unwrap();
    let store: Arc<dyn MappingStore + Send + Sync> = if mode == "sqlite" {
        Arc::new(SqliteStore::open(temp.path().join("probe.db"), &[7u8; 32]).unwrap())
    } else {
        Arc::new(MemoryStore::new())
    };
    // Cold insertion is measured separately; warm measurements reuse one shared store.
    let cold_start = Instant::now();
    if mode != "detect" {
        for t in texts.iter() {
            pipeline(t, &rs, store.as_ref(), false);
        }
    }
    let cold_ns = cold_start.elapsed().as_nanos();
    for t in texts.iter() {
        std::hint::black_box(rs.find_hits(t));
    }
    let gate = Arc::new(Barrier::new(threads + 1));
    let handles: Vec<_> = (0..threads)
        .map(|worker| {
            let (texts, rs, store, mode, gate) = (
                texts.clone(),
                rs.clone(),
                store.clone(),
                mode.clone(),
                gate.clone(),
            );
            std::thread::spawn(move || {
                gate.wait();
                let start = Instant::now();
                let mut samples = Vec::new();
                let mut i = worker;
                let mut chars = 0usize;
                while start.elapsed() < duration {
                    let text = &texts[i % texts.len()];
                    let now = Instant::now();
                    if mode == "detect" {
                        std::hint::black_box(rs.find_hits(text));
                    } else {
                        std::hint::black_box(pipeline(text, &rs, store.as_ref(), false));
                    }
                    samples.push(now.elapsed().as_nanos() as u64);
                    chars += text.chars().count();
                    i += 1;
                }
                (samples, chars)
            })
        })
        .collect();
    let start = Instant::now();
    gate.wait();
    let mut samples = Vec::new();
    let mut chars = 0;
    for h in handles {
        let (s, c) = h.join().unwrap();
        samples.extend(s);
        chars += c;
    }
    let wall = start.elapsed().as_secs_f64();
    json!({"samples_ns":samples,"wall_seconds":wall,"characters":chars,"cold_pass_ns":cold_ns,"cold_records":texts.len()})
}

fn main() {
    for line in io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        println!("{}", run(v));
    }
}
