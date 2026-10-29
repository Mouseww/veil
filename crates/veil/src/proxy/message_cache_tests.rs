use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use veil_engine::builtin::builtin_ruleset;
use veil_engine::creator::anonymous_creator;
use veil_engine::mapping::MemoryStore;
use veil_engine::rules::{Allowlist, Rule};
use veil_engine::walk::restore_json;

#[derive(Default)]
struct Store {
    inner: Mutex<MemoryStore>,
    calls: AtomicUsize,
    batches: AtomicUsize,
    fail: AtomicBool,
}
impl MappingStore for Store {
    fn get_or_insert(&self, c: &Creator, t: &str, text: &str) -> Result<Placeholder, StoreError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail.load(Ordering::Relaxed) {
            return Err(StoreError::Unavailable);
        }
        self.inner.lock().unwrap().get_or_insert(c, t, text)
    }
    fn lookup(&self, c: &Creator, p: &Placeholder) -> Result<Lookup, StoreError> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(StoreError::Unavailable);
        }
        self.inner.lock().unwrap().lookup(c, p)
    }
    fn validate_and_touch(&self, c: &Creator, ps: &[Placeholder]) -> Result<Vec<bool>, StoreError> {
        self.batches.fetch_add(1, Ordering::Relaxed);
        ps.iter()
            .map(|p| self.lookup(c, p).map(|v| matches!(v, Lookup::Hit(_))))
            .collect()
    }
}

#[test]
fn caches_chat_and_responses_items_but_still_scans_top_level_fields() {
    let (cache, store, rules, c) = (
        MessageCache::default(),
        Store::default(),
        builtin_ruleset(),
        anonymous_creator(),
    );
    let input = json!({
        "messages":[{"role":"user","content":"13800138000"}],
        "input":[{"type":"function_call_output","call_id":"call-a","output":"8.8.8.8"}],
        "system":"13900139000", "instructions":"Asia/Shanghai",
        "tools":[{"name":"lookup","description":"password=secret123"}]
    });
    let first = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    let calls = store.calls.load(Ordering::Relaxed);
    let second = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        second,
        desensitize_json(&input, &rules, &store, &c).unwrap()
    );
    assert_eq!(restore_json(&second, &store, &c).unwrap(), input);
    assert_eq!(cache.stats().hits, 2);
    assert_eq!(store.batches.load(Ordering::Relaxed), 1);
    assert!(store.calls.load(Ordering::Relaxed) > calls); // top-level strings still run
    for entry in cache.entries.lock().unwrap().values.values() {
        let text = std::str::from_utf8(&entry.value.encoded).unwrap();
        assert!(!text.contains("13800138000") && !text.contains("8.8.8.8"));
    }
}

#[test]
fn complete_message_metadata_and_new_assistant_content_cannot_hit_old_entries() {
    let (cache, store, rules, c) = (
        MessageCache::default(),
        Store::default(),
        builtin_ruleset(),
        anonymous_creator(),
    );
    let mut input =
        json!({"messages":[{"role":"user","content":"13800138000","tool_call_id":"a"}]});
    cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    for (field, value) in [
        ("role", "assistant"),
        ("tool_call_id", "b"),
        ("content", "13900139000"),
    ] {
        input["messages"][0][field] = json!(value);
        let result = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
        assert_eq!(
            result,
            desensitize_json(&input, &rules, &store, &c).unwrap()
        );
        assert_eq!(cache.stats().hits, 0);
    }
    input["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"assistant","content":"10.0.0.5"}));
    let result = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_eq!(cache.stats().hits, 1);
    assert!(!result["messages"][1]["content"]
        .as_str()
        .unwrap()
        .contains("10.0.0.5"));
}

#[test]
fn creator_rules_and_allowlist_are_isolated() {
    let (cache, store, c) = (
        MessageCache::default(),
        Store::default(),
        anonymous_creator(),
    );
    let rules = RuleSet::new(
        vec![Rule::dictionary("x", "FIRST", vec!["secret".into()], 1)],
        Allowlist::default(),
    );
    let input = json!({"messages":[{"content":"secret"}]});
    let first = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    let other = Creator::from_secret("other-user");
    let second = cache
        .desensitize_json(&input, &rules, &store, &other)
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(cache.stats().hits, 0);
    let allowed = RuleSet::new(rules.rules().to_vec(), Allowlist::from(["secret"]));
    assert_ne!(rules.cache_fingerprint(), allowed.cache_fingerprint());
    assert_eq!(
        cache
            .desensitize_json(&input, &allowed, &store, &c)
            .unwrap(),
        input
    );
    let disabled = RuleSet::new(Vec::new(), Allowlist::default());
    assert_eq!(
        cache
            .desensitize_json(&input, &disabled, &store, &c)
            .unwrap(),
        input
    );
}

#[test]
fn missing_mappings_are_rebuilt_even_without_generation_notifications() {
    let (cache, store, rules, c) = (
        MessageCache::default(),
        Store::default(),
        builtin_ruleset(),
        anonymous_creator(),
    );
    let input = json!({"messages":[{"content":"13800138000"}]});
    let before = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    *store.inner.lock().unwrap() = MemoryStore::new();
    let after = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_ne!(before, after);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(restore_json(&after, &store, &c).unwrap(), input);
    assert_eq!(
        cache.desensitize_json(&input, &rules, &store, &c).unwrap(),
        after
    );
    assert_eq!(cache.stats().hits, 1);
}

#[test]
fn cached_mappings_do_not_hide_store_errors_or_capture_literal_placeholders() {
    let (cache, store, rules, c) = (
        MessageCache::default(),
        Store::default(),
        builtin_ruleset(),
        anonymous_creator(),
    );
    let input =
        json!({"messages":[{"content":"{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4AA}} 13800138000"}]});
    cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_eq!(
        cache
            .entries
            .lock()
            .unwrap()
            .values
            .values()
            .next()
            .unwrap()
            .value
            .mappings
            .len(),
        1
    );
    cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_eq!(cache.stats().hits, 1);
    store.fail.store(true, Ordering::Relaxed);
    assert!(matches!(
        cache.desensitize_json(&input, &rules, &store, &c),
        Err(WalkError::StoreUnavailable)
    ));
    let plain = json!({"messages":[{"content":"ordinary text"}]});
    assert_eq!(
        cache.desensitize_json(&plain, &rules, &store, &c).unwrap(),
        plain
    );
}

#[test]
fn budget_lru_expiry_and_disabled_cache_fall_back_to_processing() {
    let (store, rules, c) = (MemoryStore::new(), builtin_ruleset(), anonymous_creator());
    let mut cache = MessageCache::default();
    cache.capacity = 4096;
    cache.max_entry = 4096;
    let inputs: Vec<_> = ["A", "B", "C"].into_iter().map(|tag| json!({"messages":[{"content":format!("{tag} {}", "ordinary text. ".repeat(40))}]})).collect();
    for index in [0, 1, 0, 2, 0] {
        assert_eq!(
            cache
                .desensitize_json(&inputs[index], &rules, &store, &c)
                .unwrap(),
            inputs[index]
        );
        assert!(cache.stats().charged_bytes <= 4096);
    }
    assert_eq!(cache.stats().hits, 2);
    let misses = cache.stats().misses;
    cache
        .desensitize_json(&inputs[1], &rules, &store, &c)
        .unwrap();
    assert_eq!(cache.stats().misses, misses + 1); // B was least recently used
    {
        let mut entries = cache.entries.lock().unwrap();
        for entry in entries.values.values_mut() {
            entry.used_at = Instant::now() - Duration::from_secs(601);
        }
    }
    assert_eq!(cache.stats().entries, 0);
    cache.max_entry = 100;
    cache
        .desensitize_json(&inputs[0], &rules, &store, &c)
        .unwrap();
    assert_eq!(cache.stats().entries, 0);
    let disabled = MessageCache::new(&MessageCacheConfig {
        enabled: false,
        ..MessageCacheConfig::default()
    });
    assert_eq!(
        disabled
            .desensitize_json(&inputs[0], &rules, &store, &c)
            .unwrap(),
        inputs[0]
    );
    assert_eq!(disabled.stats().entries, 0);
}

#[test]
fn concurrent_requests_share_entries_without_holding_cache_lock_during_store_calls() {
    let (cache, store, rules) = (
        Arc::new(MessageCache::default()),
        Arc::new(MemoryStore::new()),
        Arc::new(builtin_ruleset()),
    );
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let (cache, store, rules) = (cache.clone(), store.clone(), rules.clone());
            std::thread::spawn(move || {
                let input = json!({"input":[{"role":"user","content":"13800138000 and 8.8.8.8"}]});
                (0..5)
                    .map(|_| {
                        cache
                            .desensitize_json(&input, &rules, store.as_ref(), &anonymous_creator())
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let outputs: Vec<_> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    assert!(outputs.iter().all(|out| out == &outputs[0]));
    assert!(cache.stats().hits > 0);
    assert_eq!(cache.stats().entries, 1);
}

#[test]
fn sqlite_purge_and_store_replacement_invalidate_cached_messages() {
    let dir = tempfile::tempdir().unwrap();
    let store = veil_store::SqliteStore::open(dir.path().join("cache.db"), &[7; 32])
        .unwrap()
        .with_ttl(Duration::ZERO);
    let (cache, rules, c) = (
        MessageCache::default(),
        builtin_ruleset(),
        anonymous_creator(),
    );
    let input = json!({"messages":[{"content":"13800138000"}]});
    let before = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    store.purge().unwrap();
    let after = cache.desensitize_json(&input, &rules, &store, &c).unwrap();
    assert_ne!(before, after);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(restore_json(&after, &store, &c).unwrap(), input);
    let replacement = MemoryStore::new();
    let replaced = cache
        .desensitize_json(&input, &rules, &replacement, &c)
        .unwrap();
    assert_ne!(after, replaced);
    assert_eq!(restore_json(&replaced, &replacement, &c).unwrap(), input);
}

#[test]
fn poisoned_cache_and_unrecognized_json_shapes_use_the_original_engine() {
    let cache = Arc::new(MessageCache::default());
    let poison = cache.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poison.entries.lock().unwrap();
        panic!("poison test");
    })
    .join();
    let (store, rules, c) = (MemoryStore::new(), builtin_ruleset(), anonymous_creator());
    for input in [
        json!({"messages":[{"content":"13800138000"}]}),
        json!(["13800138000"]),
        json!({"input":"13800138000"}),
        json!({"messages":null,"data":"A".repeat(300)}),
    ] {
        assert_eq!(
            cache.desensitize_json(&input, &rules, &store, &c).unwrap(),
            desensitize_json(&input, &rules, &store, &c).unwrap()
        );
    }
}
