//! Offline experiment, not a production cache. All fixtures are synthetic.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    hint::black_box,
    time::Instant,
};
use veil_engine::{
    builtin::builtin_ruleset,
    creator::{anonymous_creator, Creator},
    rules::RuleSet,
    walk::{desensitize_json, restore_json},
};
use veil_store::SqliteStore;

const MB: usize = 1_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Scope {
    creator: Creator,
    rules_generation: u64,
    mappings_generation: u64,
}

// Only SHA-256 fingerprints and masked messages are retained. The caller must
// invalidate the mapping generation on expiry/purge/rotation; this example does
// not add any cache or invalidation hooks to the production proxy.
#[derive(Default)]
struct HistoryCache {
    scope: Option<Scope>,
    messages: HashMap<[u8; 32], Value>,
}

#[derive(Default)]
struct Stats {
    hit_messages: usize,
    miss_messages: usize,
    hit_bytes: usize,
    miss_bytes: usize,
    key_ns: u64,
    clone_ns: u64,
    redact_ns: u64,
    retained_bytes: usize,
}

impl HistoryCache {
    fn process(
        &mut self,
        input: &Value,
        rules: &RuleSet,
        store: &SqliteStore,
        scope: Scope,
    ) -> (Value, Stats) {
        if self.scope != Some(scope) {
            self.messages.clear();
            self.scope = Some(scope);
        }
        let mut out = serde_json::Map::new();
        let mut stats = Stats::default();
        let mut used = HashSet::new();
        // Keep the complete message object in the fingerprint, including role,
        // tool fields and attachments. Other top-level fields are always scanned.
        for (key, value) in input.as_object().unwrap() {
            if key != "messages" {
                out.insert(
                    key.clone(),
                    desensitize_json(&json!({key: value}), rules, store, &scope.creator).unwrap()
                        [key]
                        .clone(),
                );
                continue;
            }
            let mut messages = Vec::new();
            for message in value.as_array().unwrap() {
                let start = Instant::now();
                let digest: [u8; 32] = Sha256::digest(serde_json::to_vec(message).unwrap()).into();
                stats.key_ns += ns(start);
                used.insert(digest);
                let bytes = message["content"].as_str().map_or(0, str::len);
                if let Some(masked) = self.messages.get(&digest) {
                    let start = Instant::now();
                    messages.push(masked.clone());
                    stats.clone_ns += ns(start);
                    stats.hit_messages += 1;
                    stats.hit_bytes += bytes;
                } else {
                    let start = Instant::now();
                    let masked = desensitize_json(message, rules, store, &scope.creator).unwrap();
                    stats.redact_ns += ns(start);
                    let start = Instant::now();
                    self.messages.insert(digest, masked.clone());
                    stats.clone_ns += ns(start);
                    messages.push(masked);
                    stats.miss_messages += 1;
                    stats.miss_bytes += bytes;
                }
            }
            out.insert(key.clone(), Value::Array(messages));
        }
        self.messages.retain(|digest, _| used.contains(digest));
        stats.retained_bytes = self
            .messages
            .values()
            .map(|m| m["content"].as_str().map_or(0, str::len))
            .sum();
        (Value::Object(out), stats)
    }
}

fn ns(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}

pub(crate) fn message(id: usize, role: &str, stride: usize) -> Value {
    let filler = "This document describes routine project planning and implementation details. ";
    let mut text = String::with_capacity(MB);
    let mut block = 0;
    if stride != 0 {
        while text.len() + stride <= MB {
            let end = text.len() + stride;
            let value = id * 10_000 + block;
            text.push_str(&format!(" Contact 138{value:08} host 10.{}.{}.{} credential sk-history{value:012} timezone Asia/Shanghai. ", (value / 65536) % 256, (value / 256) % 256, value % 256));
            while text.len() + filler.len() <= end {
                text.push_str(filler);
            }
            text.extend(std::iter::repeat_n(' ', end - text.len()));
            block += 1;
        }
    }
    // A unique non-sensitive suffix makes even the no-PII messages change per turn.
    text.push_str(&format!(" Message sequence {id:08}. "));
    while text.len() + filler.len() <= MB {
        text.push_str(filler);
    }
    text.extend(std::iter::repeat_n(' ', MB - text.len()));
    assert_eq!(text.len(), MB);
    json!({"role":role,"content":text})
}

fn full(wire: &[u8], rules: &RuleSet, store: &SqliteStore, creator: &Creator) -> (Vec<u8>, Value) {
    let input: Value = serde_json::from_slice(wire).unwrap();
    let masked = desensitize_json(&input, rules, store, creator).unwrap();
    (serde_json::to_vec(&masked).unwrap(), masked)
}

fn cached(
    wire: &[u8],
    cache: &mut HistoryCache,
    rules: &RuleSet,
    store: &SqliteStore,
    scope: Scope,
) -> (Vec<u8>, Value, Stats, u64, u64) {
    let start = Instant::now();
    let input: Value = serde_json::from_slice(wire).unwrap();
    let parse_ns = ns(start);
    let (masked, stats) = cache.process(&input, rules, store, scope);
    let start = Instant::now();
    let output = serde_json::to_vec(&masked).unwrap();
    let encode_ns = ns(start);
    (output, masked, stats, parse_ns, encode_ns)
}

fn validate_invalidation(rules: &RuleSet) {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(temp.path().join("invalidation.db"), &[7; 32])
        .unwrap()
        .with_ttl(std::time::Duration::ZERO);
    let mut cache = HistoryCache::default();
    let scope = Scope {
        creator: anonymous_creator(),
        rules_generation: 1,
        mappings_generation: 1,
    };
    let mut input = json!({"messages":[{"role":"user","content":"Call 13800138000"}]});
    let (_, stats) = cache.process(&input, rules, &store, scope);
    assert_eq!(stats.miss_messages, 1);
    let (_, stats) = cache.process(&input, rules, &store, scope);
    assert_eq!(stats.hit_messages, 1);
    input["messages"][0]["content"] = json!("Changed 13900139000");
    let (_, stats) = cache.process(&input, rules, &store, scope);
    assert_eq!(stats.miss_messages, 1);
    input["messages"][0]["role"] = json!("assistant");
    let (_, stats) = cache.process(&input, rules, &store, scope);
    assert_eq!(stats.miss_messages, 1);
    input["messages"][0]["tool_call_id"] = json!("changed-call");
    let (_, stats) = cache.process(&input, rules, &store, scope);
    assert_eq!(stats.miss_messages, 1);
    for changed in [
        Scope {
            creator: Creator::from_secret("other"),
            ..scope
        },
        Scope {
            rules_generation: 2,
            ..scope
        },
        Scope {
            mappings_generation: 2,
            ..scope
        },
    ] {
        let (masked, stats) = cache.process(&input, rules, &store, changed);
        assert_eq!(stats.miss_messages, 1);
        assert_eq!(
            masked,
            desensitize_json(&input, rules, &store, &changed.creator).unwrap()
        );
        // Reset so each scope field is tested independently.
        cache.process(&input, rules, &store, scope);
    }
    let disabled = RuleSet::new(Vec::new(), veil_engine::rules::Allowlist::default());
    let changed_rules = Scope {
        rules_generation: 2,
        ..scope
    };
    let (masked, stats) = cache.process(&input, &disabled, &store, changed_rules);
    assert_eq!(stats.miss_messages, 1);
    assert_eq!(masked, input);
    let (before_purge, _) = cache.process(&input, rules, &store, scope);
    store.purge().unwrap();
    let (after_purge, stats) = cache.process(
        &input,
        rules,
        &store,
        Scope {
            mappings_generation: 2,
            ..scope
        },
    );
    assert_eq!(stats.miss_messages, 1);
    assert_ne!(before_purge, after_purge);
    assert_eq!(
        restore_json(&after_purge, &store, &scope.creator).unwrap(),
        input
    );
    println!(
        "{}",
        json!({"kind":"validation","content_edit":true,"role_edit":true,"tool_field_edit":true,"creator_change":true,"rule_generation_change":true,"mapping_generation_change":true,"actual_rule_change":true,"actual_mapping_purge":true})
    );
}

fn main() {
    let turns: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "20".into())
        .parse()
        .unwrap();
    assert!(turns >= 2);
    let rules = builtin_ruleset();
    validate_invalidation(&rules);
    for (profile, stride) in [("none", 0), ("sparse", 65536), ("dense", 4096)] {
        for scenario in ["stable", "rolling"] {
            let temp = tempfile::tempdir().unwrap();
            let store = SqliteStore::open(temp.path().join("history.db"), &[7; 32]).unwrap();
            let scope = Scope {
                creator: anonymous_creator(),
                rules_generation: 1,
                mappings_generation: 1,
            };
            let mut history: Vec<Value> = (0..9)
                .map(|id| message(id, if id % 2 == 0 { "assistant" } else { "user" }, stride))
                .collect();
            let history_request = json!({"messages":history});
            // Warm mappings before both paths, so pair order cannot favor the
            // second path with fresh insertions. Cache fill is measured separately.
            desensitize_json(&history_request, &rules, &store, &scope.creator).unwrap();
            let mut cache = HistoryCache::default();
            let start = Instant::now();
            let (masked_history, _) = cache.process(&history_request, &rules, &store, scope);
            let prime_ns = ns(start);
            assert_eq!(
                restore_json(&masked_history, &store, &scope.creator).unwrap(),
                history_request
            );
            for turn in 0..turns {
                let current = message(100 + turn * 2, "user", stride);
                desensitize_json(&current, &rules, &store, &scope.creator).unwrap();
                // A rolling conversation adds a fresh assistant reply every turn.
                // Prewarming its mappings does NOT put that reply in the cache.
                if scenario == "rolling" && turn > 0 {
                    desensitize_json(history.last().unwrap(), &rules, &store, &scope.creator)
                        .unwrap();
                }
                let mut messages = history.clone();
                messages.push(current.clone());
                let request = json!({"model":"offline-benchmark","messages":messages});
                let wire = serde_json::to_vec(&request).unwrap();
                let (baseline, reused, full_ns, cache_ns) = if turn % 2 == 0 {
                    let start = Instant::now();
                    let baseline = full(&wire, &rules, &store, &scope.creator);
                    let full_ns = ns(start);
                    let start = Instant::now();
                    let reused = cached(&wire, &mut cache, &rules, &store, scope);
                    let cache_ns = ns(start);
                    (baseline, reused, full_ns, cache_ns)
                } else {
                    let start = Instant::now();
                    let reused = cached(&wire, &mut cache, &rules, &store, scope);
                    let cache_ns = ns(start);
                    let start = Instant::now();
                    let baseline = full(&wire, &rules, &store, &scope.creator);
                    let full_ns = ns(start);
                    (baseline, reused, full_ns, cache_ns)
                };
                assert_eq!(baseline.0, reused.0);
                assert_eq!(baseline.1, reused.1);
                assert_eq!(
                    restore_json(&reused.1, &store, &scope.creator).unwrap(),
                    request
                );
                let stats = reused.2;
                let expected_hits = if scenario == "rolling" && turn > 0 {
                    8
                } else {
                    9
                };
                assert_eq!(stats.hit_messages, expected_hits);
                assert_eq!(stats.miss_messages, 10 - expected_hits);
                assert_eq!(stats.hit_bytes + stats.miss_bytes, 10 * MB);
                let expected_pii = if stride == 0 {
                    0
                } else {
                    10 * (MB / stride) * 4
                };
                let placeholders: usize = reused.1["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m["content"].as_str().unwrap().matches("{{").count())
                    .sum();
                assert_eq!(placeholders, expected_pii);
                black_box(&reused.0);
                println!(
                    "{}",
                    json!({"kind":"sample","profile":profile,"scenario":scenario,"turn":turn+1,"wire_bytes":wire.len(),"history_content_bytes":9*MB,"new_user_content_bytes":MB,"full_ns":full_ns,"cache_ns":cache_ns,"cache_prime_ns":prime_ns,"hit_messages":stats.hit_messages,"miss_messages":stats.miss_messages,"hit_bytes":stats.hit_bytes,"miss_bytes":stats.miss_bytes,"key_ns":stats.key_ns,"clone_ns":stats.clone_ns,"redact_ns":stats.redact_ns,"parse_ns":reused.3,"encode_ns":reused.4,"retained_masked_content_bytes":stats.retained_bytes,"retained_messages":cache.messages.len(),"placeholders":placeholders,"outputs_equal":true,"roundtrip_ok":true,"order":if turn%2==0 {"full_first"} else {"cache_first"}})
                );
                if scenario == "rolling" {
                    history.drain(..2);
                    history.push(current);
                    history.push(message(101 + turn * 2, "assistant", stride));
                }
            }
            eprintln!("completed {profile} {scenario}, {turns} paired turns");
        }
    }
}
