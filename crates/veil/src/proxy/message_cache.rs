use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};
use veil_engine::creator::Creator;
use veil_engine::mapping::{Lookup, MappingStore, StoreError};
use veil_engine::placeholder::Placeholder;
use veil_engine::rules::RuleSet;
use veil_engine::walk::{desensitize_json, WalkError};

use crate::config::MessageCacheConfig;

const MAX_ENTRIES: usize = 4096;

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    creator: Creator,
    policy: [u8; 32],
    rules_revision: u64,
    store_identity: usize,
    generation: u64,
    kind: u8,
    digest: [u8; 32],
}

struct Cached {
    encoded: Box<[u8]>,
    mappings: Box<[Placeholder]>,
}

impl Cached {
    fn weight(&self) -> usize {
        // Include payload, mapping strings, keys, index nodes and allocator
        // overhead in the budget. This is not a process RSS limit.
        self.encoded
            .len()
            .saturating_add(self.mappings.len().saturating_mul(size_of::<Placeholder>()))
            .saturating_add(
                self.mappings
                    .iter()
                    .map(|p| p.type_prefix.capacity())
                    .sum::<usize>(),
            )
            .saturating_add(2 * size_of::<Key>() + size_of::<Entry>() + size_of::<Cached>() + 512)
    }
}

struct Entry {
    value: Arc<Cached>,
    used_at: Instant,
    tick: u64,
    weight: usize,
}

#[derive(Default)]
struct Entries {
    values: HashMap<Key, Entry>,
    order: BTreeMap<u64, Key>,
    tick: u64,
    bytes: usize,
}

impl Entries {
    fn remove(&mut self, key: &Key) {
        if let Some(entry) = self.values.remove(key) {
            self.bytes -= entry.weight;
            self.order.remove(&entry.tick);
        }
    }

    fn expire(&mut self, now: Instant, ttl: Duration) {
        while let Some((_, key)) = self.order.first_key_value() {
            if now.saturating_duration_since(self.values[key].used_at) < ttl {
                break;
            }
            let key = key.clone();
            self.remove(&key);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MessageCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub charged_bytes: usize,
}

/// Process-local cache shared by proxy routes. It retains fingerprints and
/// serialized redacted messages, never an additional copy of the raw history.
pub struct MessageCache {
    capacity: usize,
    max_entry: usize,
    ttl: Duration,
    entries: Mutex<Entries>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for MessageCache {
    fn default() -> Self {
        Self::new(&MessageCacheConfig::default())
    }
}

impl MessageCache {
    pub fn new(config: &MessageCacheConfig) -> Self {
        let bytes = |mib: u64| mib.saturating_mul(1024 * 1024).min(usize::MAX as u64) as usize;
        let capacity = if config.enabled {
            bytes(config.capacity_mib)
        } else {
            0
        };
        Self {
            capacity,
            max_entry: bytes(config.max_entry_mib).min(capacity),
            ttl: Duration::from_secs(config.idle_ttl_seconds),
            entries: Mutex::new(Entries::default()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn stats(&self) -> MessageCacheStats {
        let mut stats = MessageCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            ..MessageCacheStats::default()
        };
        if let Ok(mut entries) = self.entries.lock() {
            entries.expire(Instant::now(), self.ttl);
            stats.entries = entries.values.len();
            stats.charged_bytes = entries.bytes;
        }
        stats
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            *entries = Entries::default();
        }
    }

    /// Release idle entries even when no requests are arriving.
    pub fn expire_idle(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.expire(Instant::now(), self.ttl);
        }
    }

    fn get(&self, key: &Key) -> Option<Arc<Cached>> {
        let mut entries = self.entries.lock().ok()?;
        let now = Instant::now();
        entries.expire(now, self.ttl);
        let old_tick = entries.values.get(key)?.tick;
        entries.order.remove(&old_tick);
        entries.tick += 1;
        let tick = entries.tick;
        let entry = entries.values.get_mut(key)?;
        entry.tick = tick;
        entry.used_at = now;
        let value = entry.value.clone();
        entries.order.insert(tick, key.clone());
        Some(value)
    }

    fn insert(&self, key: Key, value: Cached) {
        let weight = value.weight();
        if weight > self.max_entry || weight > self.capacity {
            return;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = Instant::now();
        entries.expire(now, self.ttl);
        entries.remove(&key);
        while entries.bytes > self.capacity - weight || entries.values.len() >= MAX_ENTRIES {
            let Some((_, oldest)) = entries.order.first_key_value() else {
                break;
            };
            let oldest = oldest.clone();
            entries.remove(&oldest);
        }
        entries.tick += 1;
        let tick = entries.tick;
        entries.bytes += weight;
        entries.order.insert(tick, key.clone());
        entries.values.insert(
            key,
            Entry {
                value: Arc::new(value),
                used_at: now,
                tick,
                weight,
            },
        );
    }

    /// Same JSON matching semantics as desensitize_json. Only complete message
    /// items are reusable; changed, missing or expired items follow the engine.
    pub fn desensitize_json(
        &self,
        input: &Value,
        rules: &RuleSet,
        store: &(impl MappingStore + ?Sized),
        creator: &Creator,
    ) -> Result<Value, WalkError> {
        let Some(object) = input.as_object() else {
            return desensitize_json(input, rules, store, creator);
        };
        if self.max_entry == 0 || self.ttl.is_zero() {
            return desensitize_json(input, rules, store, creator);
        }
        if !object
            .iter()
            .any(|(field, value)| kind(field).is_some() && value.is_array())
        {
            return desensitize_json(input, rules, store, creator);
        }
        let generation = store.cache_generation();
        let template = Key {
            creator: *creator,
            policy: rules.cache_fingerprint(),
            rules_revision: rules.cache_revision(),
            store_identity: store as *const _ as *const () as usize,
            generation,
            kind: 0,
            digest: [0; 32],
        };
        let mut candidates = HashMap::new();
        let mut references = HashMap::new();
        for (field, value) in object {
            let Some(kind) = kind(field) else {
                continue;
            };
            if let Some(items) = value.as_array() {
                for (index, item) in items.iter().enumerate() {
                    if candidates.len() >= MAX_ENTRIES {
                        break;
                    }
                    if let Some(candidate) = self.candidate(item, kind, &template) {
                        if let Some(cached) = &candidate.1 {
                            for p in cached.mappings.iter() {
                                references.entry(p.clone()).or_insert(false);
                            }
                        }
                        candidates.insert((kind, index), candidate);
                    }
                }
            }
        }
        let ids: Vec<_> = references.keys().cloned().collect();
        if !ids.is_empty() {
            let valid = store
                .validate_and_touch(creator, &ids)
                .map_err(|_| WalkError::StoreUnavailable)?;
            if valid.len() != ids.len() {
                return Err(WalkError::StoreUnavailable);
            }
            for (id, valid) in ids.into_iter().zip(valid) {
                references.insert(id, valid);
            }
        }
        let mut out = serde_json::Map::new();
        // Iterate in the same object/array order as the uncached engine, keeping
        // first-insertion mapping type reuse semantics for cache misses.
        for (field, value) in object {
            if let Some((kind, items)) = kind(field).zip(value.as_array()) {
                let mut masked = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let candidate = candidates.remove(&(kind, index));
                    let hit = candidate
                        .as_ref()
                        .and_then(|(_, cached)| cached.as_ref())
                        .filter(|cached| {
                            cached
                                .mappings
                                .iter()
                                .all(|p| references.get(p) == Some(&true))
                        });
                    if let Some(value) =
                        hit.and_then(|cached| serde_json::from_slice(&cached.encoded).ok())
                    {
                        self.hits.fetch_add(1, Ordering::Relaxed);
                        masked.push(value);
                        continue;
                    }
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    let recorder = RecordingStore {
                        inner: store,
                        mappings: RefCell::new(Vec::new()),
                    };
                    let value = desensitize_json(item, rules, &recorder, creator)?;
                    if let Some((key, _)) = candidate {
                        let mut mappings = recorder.mappings.into_inner();
                        let mut seen = std::collections::HashSet::new();
                        mappings.retain(|p| seen.insert(p.clone()));
                        if let Ok(encoded) = serde_json::to_vec(&value) {
                            if store.cache_generation() == generation {
                                self.insert(
                                    key,
                                    Cached {
                                        encoded: encoded.into_boxed_slice(),
                                        mappings: mappings.into_boxed_slice(),
                                    },
                                );
                            }
                        }
                    }
                    masked.push(value);
                }
                out.insert(field.clone(), Value::Array(masked));
            } else {
                // Preserve the parent key's binary-payload skipping behavior.
                let wrapper =
                    Value::Object(serde_json::Map::from_iter([(field.clone(), value.clone())]));
                let mut masked = desensitize_json(&wrapper, rules, store, creator)?;
                out.insert(
                    field.clone(),
                    masked.as_object_mut().unwrap().remove(field).unwrap(),
                );
            }
        }
        if store.cache_generation() != generation {
            return desensitize_json(input, rules, store, creator);
        }
        Ok(Value::Object(out))
    }

    fn candidate(
        &self,
        item: &Value,
        kind: u8,
        template: &Key,
    ) -> Option<(Key, Option<Arc<Cached>>)> {
        let mut writer = Fingerprint {
            hash: Sha256::new(),
            bytes: 0,
        };
        serde_json::to_writer(&mut writer, item).ok()?;
        if writer.bytes > self.max_entry {
            return None;
        }
        let key = Key {
            kind,
            digest: writer.hash.finalize().into(),
            ..template.clone()
        };
        let value = self.get(&key);
        Some((key, value))
    }
}

fn kind(field: &str) -> Option<u8> {
    match field {
        "messages" => Some(1),
        "input" => Some(2),
        _ => None,
    }
}

struct Fingerprint {
    hash: Sha256,
    bytes: usize,
}
impl Write for Fingerprint {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.hash.update(bytes);
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct RecordingStore<'a, S: MappingStore + ?Sized> {
    inner: &'a S,
    mappings: RefCell<Vec<Placeholder>>,
}
impl<S: MappingStore + ?Sized> MappingStore for RecordingStore<'_, S> {
    fn get_or_insert(
        &self,
        creator: &Creator,
        type_prefix: &str,
        plaintext: &str,
    ) -> Result<Placeholder, StoreError> {
        let p = self.inner.get_or_insert(creator, type_prefix, plaintext)?;
        self.mappings.borrow_mut().push(p.clone());
        Ok(p)
    }
    fn lookup(&self, creator: &Creator, placeholder: &Placeholder) -> Result<Lookup, StoreError> {
        self.inner.lookup(creator, placeholder)
    }
}

#[cfg(test)]
#[path = "message_cache_tests.rs"]
mod tests;
