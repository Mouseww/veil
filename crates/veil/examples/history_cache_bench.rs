//! Compare the production message cache against the original JSON pipeline.
#[allow(dead_code)]
#[path = "../../veil-store/examples/history_reuse_bench.rs"]
mod fixtures;

use serde_json::{json, Value};
use std::time::Instant;
use veil::proxy::MessageCache;
use veil_engine::{
    builtin::builtin_ruleset,
    creator::{anonymous_creator, Creator},
    rules::RuleSet,
    walk::{desensitize_json, restore_json},
};
use veil_store::SqliteStore;

fn ns(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}

fn pipeline(
    wire: &[u8],
    cache: Option<&MessageCache>,
    rules: &RuleSet,
    store: &SqliteStore,
    creator: &Creator,
) -> (Vec<u8>, Value) {
    let input: Value = serde_json::from_slice(wire).unwrap();
    let masked = match cache {
        Some(cache) => cache
            .desensitize_json(&input, rules, store, creator)
            .unwrap(),
        None => desensitize_json(&input, rules, store, creator).unwrap(),
    };
    (serde_json::to_vec(&masked).unwrap(), masked)
}

fn main() {
    let turns: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "20".into())
        .parse()
        .unwrap();
    assert!(turns >= 2);
    let rules = builtin_ruleset();
    let creator = anonymous_creator();
    for (profile, stride) in [("none", 0), ("sparse", 65536), ("dense", 4096)] {
        for scenario in ["stable", "rolling"] {
            let temp = tempfile::tempdir().unwrap();
            let store = SqliteStore::open(temp.path().join("production.db"), &[7; 32]).unwrap();
            let cache = MessageCache::default();
            let mut history: Vec<_> = (0..9)
                .map(|id| {
                    fixtures::message(id, if id % 2 == 0 { "assistant" } else { "user" }, stride)
                })
                .collect();
            let history_request = json!({"input":history});
            desensitize_json(&history_request, &rules, &store, &creator).unwrap();
            let start = Instant::now();
            cache
                .desensitize_json(&history_request, &rules, &store, &creator)
                .unwrap();
            let prime_ns = ns(start);
            for turn in 0..turns {
                let current = fixtures::message(100 + turn * 2, "user", stride);
                desensitize_json(&current, &rules, &store, &creator).unwrap();
                if scenario == "rolling" && turn > 0 {
                    desensitize_json(history.last().unwrap(), &rules, &store, &creator).unwrap();
                }
                let mut messages = history.clone();
                messages.push(current.clone());
                let request = json!({"model":"offline-benchmark","input":messages});
                let wire = serde_json::to_vec(&request).unwrap();
                let before = cache.stats();
                let (full, reused, full_ns, cache_ns) = if turn % 2 == 0 {
                    let start = Instant::now();
                    let full = pipeline(&wire, None, &rules, &store, &creator);
                    let full_ns = ns(start);
                    let start = Instant::now();
                    let reused = pipeline(&wire, Some(&cache), &rules, &store, &creator);
                    let cache_ns = ns(start);
                    (full, reused, full_ns, cache_ns)
                } else {
                    let start = Instant::now();
                    let reused = pipeline(&wire, Some(&cache), &rules, &store, &creator);
                    let cache_ns = ns(start);
                    let start = Instant::now();
                    let full = pipeline(&wire, None, &rules, &store, &creator);
                    let full_ns = ns(start);
                    (full, reused, full_ns, cache_ns)
                };
                assert_eq!(full.0, reused.0);
                assert_eq!(full.1, reused.1);
                assert_eq!(restore_json(&reused.1, &store, &creator).unwrap(), request);
                let after = cache.stats();
                let hits = after.hits - before.hits;
                assert_eq!(
                    hits,
                    if scenario == "rolling" && turn > 0 {
                        8
                    } else {
                        9
                    }
                );
                assert!(after.charged_bytes <= 64 * 1024 * 1024);
                let expected = if stride == 0 {
                    0
                } else {
                    10 * (1_000_000 / stride) * 4
                };
                let placeholders: usize = reused.1["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m["content"].as_str().unwrap().matches("{{").count())
                    .sum();
                assert_eq!(placeholders, expected);
                println!(
                    "{}",
                    json!({"profile":profile,"scenario":scenario,"turn":turn+1,"full_ns":full_ns,"cache_ns":cache_ns,"prime_ns":prime_ns,"hit_messages":hits,"miss_messages":after.misses-before.misses,"charged_bytes":after.charged_bytes,"entries":after.entries,"outputs_equal":true,"roundtrip_ok":true,"placeholders":placeholders,"wire_bytes":wire.len(),"order":if turn%2==0 {"full_first"} else {"cache_first"}})
                );
                if scenario == "rolling" {
                    history.drain(..2);
                    history.push(current);
                    history.push(fixtures::message(101 + turn * 2, "assistant", stride));
                }
            }
            eprintln!("completed production {profile} {scenario}, {turns} pairs");
        }
    }
}
