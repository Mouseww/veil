//! Diagnostic decomposition for the exact request_size_bench payloads.
//! Production functions and SQLite remain unchanged. JSONL on stdout.
#[allow(dead_code)]
mod request_size_bench;

use serde_json::{json, Value};
use std::{hint::black_box, time::Instant};
use veil_engine::{
    builtin::builtin_ruleset,
    creator::anonymous_creator,
    mapping::MappingStore,
    rules::{Allowlist, RuleSet},
    walk::{desensitize_json, restore_json},
};
use veil_store::SqliteStore;

fn ns(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}

fn main() {
    let rules = builtin_ruleset();
    let creator = anonymous_creator();
    for mb in [1, 5, 10] {
        for (profile, stride) in [("none", 0), ("sparse", 65536), ("dense", 4096)] {
            let (wire, expected) = request_size_bench::payload(mb * 1_000_000, stride);
            let temp = tempfile::tempdir().unwrap();
            let store = SqliteStore::open(temp.path().join("stages.db"), &[7; 32]).unwrap();
            let input: Value = serde_json::from_slice(&wire).unwrap();
            let production = desensitize_json(&input, &rules, &store, &creator).unwrap();
            let mut samples = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                let input: Value = serde_json::from_slice(&wire).unwrap();
                let parse = ns(start);
                let text = input["messages"][0]["content"].as_str().unwrap();
                let start = Instant::now();
                assert!(!veil_engine::binary::should_skip_desensitize(
                    Some("content"),
                    text
                ));
                black_box(rules.find_hits("user"));
                let hits = rules.find_hits(text);
                let detect = ns(start);
                assert_eq!(hits.len(), expected);
                let start = Instant::now();
                // Resolve in production's reverse order to preserve store semantics.
                let replacements: Vec<_> = hits
                    .iter()
                    .rev()
                    .map(|hit| {
                        store
                            .get_or_insert(&creator, &hit.type_prefix, &hit.plaintext)
                            .unwrap()
                    })
                    .collect();
                let mapping = ns(start);
                let start = Instant::now();
                let mut masked = text.to_string();
                for (hit, placeholder) in hits.iter().rev().zip(&replacements) {
                    masked.replace_range(hit.start..hit.end, &placeholder.format());
                }
                let replace = ns(start);
                let start = Instant::now();
                let out = json!({"messages":[{"role":"user","content":masked} ]});
                black_box(serde_json::to_vec(&out).unwrap());
                let encode = ns(start);
                assert_eq!(out, production);
                // Candidate replacement algorithm: compare exact output using SAME mappings/hits.
                let start = Instant::now();
                let mut appended = String::with_capacity(text.len() + hits.len() * 40);
                let mut cursor = 0;
                for (hit, placeholder) in hits.iter().zip(replacements.iter().rev()) {
                    appended.push_str(&text[cursor..hit.start]);
                    appended.push_str(&placeholder.format());
                    cursor = hit.end;
                }
                appended.push_str(&text[cursor..]);
                let append = ns(start);
                assert_eq!(appended, out["messages"][0]["content"].as_str().unwrap());
                let start = Instant::now();
                let restored = restore_json(&out, &store, &creator).unwrap();
                let restore = ns(start);
                assert_eq!(restored, input);
                samples.push(json!({"parse_ns":parse,"detect_ns":detect,"mapping_ns":mapping,"replace_ns":replace,"encode_ns":encode,"append_ns":append,"restore_ns":restore,"total_ns":parse+detect+mapping+replace+encode}));
            }
            println!(
                "{}",
                json!({"kind":"stages","mb":mb,"profile":profile,"hits":expected,"samples":samples,"production_equal":true,"append_equal":true,"roundtrip_ok":true})
            );
            eprintln!("stages completed {mb}MB {profile}");
            if mb == 10 {
                let text = input["messages"][0]["content"].as_str().unwrap();
                for rule in rules.rules().iter().filter(|r| r.enabled) {
                    let isolated = RuleSet::new(
                        vec![rule.clone()],
                        Allowlist::from(["127.0.0.1", "localhost", "0.0.0.0", "::1"]),
                    );
                    let mut times = Vec::new();
                    for _ in 0..3 {
                        let start = Instant::now();
                        black_box(isolated.find_hits(text));
                        times.push(ns(start));
                    }
                    println!(
                        "{}",
                        json!({"kind":"isolated_rule","mb":mb,"profile":profile,"rule":rule.id,"samples_ns":times})
                    );
                }
            }
        }
    }
}
