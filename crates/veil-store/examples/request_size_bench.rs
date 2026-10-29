//! Offline request-body benchmark. Run with --release; stdout is JSONL.
use serde_json::{json, Value};
use std::hint::black_box;
use std::time::Instant;
use veil_engine::walk::{desensitize_json, restore_json};
use veil_engine::{builtin::builtin_ruleset, creator::anonymous_creator, rules::RuleSet};
use veil_store::SqliteStore;

pub(crate) fn payload(bytes: usize, stride: usize) -> (Vec<u8>, usize) {
    let overhead = serde_json::to_vec(&json!({"messages":[{"role":"user","content":""}]}))
        .unwrap()
        .len();
    let len = bytes - overhead;
    let filler = "This document describes routine project planning and implementation details. ";
    let mut text = String::with_capacity(len);
    let mut blocks = 0;
    if stride > 0 {
        while text.len() + stride <= len {
            let end = text.len() + stride;
            // Three distinct synthetic values per block, plus a repeated timezone.
            text.push_str(&format!(" Contact 138{:08} host 10.{}.{}.{} credential sk-bench{:012} timezone Asia/Shanghai. ", blocks, (blocks / 65536) % 256, (blocks / 256) % 256, blocks % 256, blocks));
            while text.len() + filler.len() <= end {
                text.push_str(filler);
            }
            text.extend(std::iter::repeat_n(' ', end - text.len()));
            blocks += 1;
        }
    }
    while text.len() + filler.len() <= len {
        text.push_str(filler);
    }
    text.extend(std::iter::repeat_n(' ', len - text.len()));
    let wire = serde_json::to_vec(&json!({"messages":[{"role":"user","content":text}]})).unwrap();
    assert_eq!(wire.len(), bytes);
    (wire, blocks * 4)
}

fn pipeline(wire: &[u8], rules: &RuleSet, store: &SqliteStore) -> Vec<u8> {
    let input: Value = serde_json::from_slice(wire).unwrap();
    let masked = desensitize_json(&input, rules, store, &anonymous_creator()).unwrap();
    serde_json::to_vec(&masked).unwrap()
}

fn main() {
    let repetitions: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "20".into())
        .parse()
        .unwrap();
    assert!(repetitions > 0);
    let rules = builtin_ruleset();
    for mb in [1, 5, 10] {
        for (profile, stride) in [("none", 0), ("sparse", 65536), ("dense", 4096)] {
            let (wire, expected) = payload(mb * 1_000_000, stride);
            let input: Value = serde_json::from_slice(&wire).unwrap();
            let text = input["messages"][0]["content"].as_str().unwrap();
            let hits = rules.find_hits(text);
            assert_eq!(
                hits.len(),
                expected,
                "incomplete detection: {mb}MB {profile}"
            );
            let temp = tempfile::tempdir().unwrap();
            let mut cold_ns = Vec::new();
            for index in 0..3 {
                let store =
                    SqliteStore::open(temp.path().join(format!("cold-{index}.db")), &[7; 32])
                        .unwrap();
                let start = Instant::now();
                let masked_wire = pipeline(&wire, &rules, &store);
                cold_ns.push(start.elapsed().as_nanos() as u64);
                let masked: Value = serde_json::from_slice(&masked_wire).unwrap();
                assert_eq!(
                    masked["messages"][0]["content"]
                        .as_str()
                        .unwrap()
                        .matches("{{")
                        .count(),
                    expected
                );
                assert_eq!(
                    restore_json(&masked, &store, &anonymous_creator()).unwrap(),
                    input
                );
            }
            let store = SqliteStore::open(temp.path().join("warm.db"), &[7; 32]).unwrap();
            black_box(pipeline(&wire, &rules, &store));
            let mut detect_ns = Vec::new();
            let mut warm_ns = Vec::new();
            for _ in 0..repetitions {
                let start = Instant::now();
                assert_eq!(black_box(rules.find_hits(text)).len(), expected);
                detect_ns.push(start.elapsed().as_nanos() as u64);
                let start = Instant::now();
                black_box(pipeline(&wire, &rules, &store));
                warm_ns.push(start.elapsed().as_nanos() as u64);
            }
            println!(
                "{}",
                json!({"mb":mb,"bytes":wire.len(),"profile":profile,"stride":stride,"hits":hits.len(),"unique_mappings":if expected == 0 {0} else {expected / 4 * 3 + 1},"detect_ns":detect_ns,"cold_ns":cold_ns,"warm_ns":warm_ns,"roundtrip_ok":true})
            );
            eprintln!("completed {mb}MB {profile}, {} hits", hits.len());
        }
    }
}
