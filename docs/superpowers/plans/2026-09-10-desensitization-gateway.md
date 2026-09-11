# Desensitization Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The operator already chose **subagent-driven execution**: one fresh subagent per task, main agent only dispatches and reviews.

**Goal:** Ship a local/internal AI desensitization reverse proxy (`dgw`) that redacts secrets in Anthropic Messages, OpenAI Chat Completions, and OpenAI Responses (including SSE restore), plus a management UI and a Claude Code plugin â€?without changing downstream request parameters.

**Architecture:** One Rust process listens on a Proxy Port (default 18787) and a Management Port (default 18788). The engine walks JSON string values, replaces hits with `{{TYPE_ULID}}` tokens, persists Creator-bound mappings in encrypted SQLite, and restores tokens on the response stream with a bounded sliding window. The Claude Code plugin starts that binary, chains any existing Base URL into the Upstream Target, and points user-level `ANTHROPIC_BASE_URL` at the Proxy Port.

**Tech Stack:** Rust 2021, Axum, Tokio, reqwest, rusqlite (bundled), aes-gcm, hkdf, sha2, ulid, serde, toml, rust-embed; Vite + React + TypeScript UI; Claude Code plugin manifests; GitHub Actions Release assets; Apache-2.0.

**Domain docs (read first, do not contradict):** `CONTEXT.md`, `docs/adr/0001-rust-gateway.md`, `docs/adr/0002-sqlite-mapping-store.md`, `docs/adr/0003-apache-2.0.md`, `docs/adr/0004-mapping-encryption.md`, `docs/adr/0005-redact-all-parseable-ips.md`.

---

## File structure

Create this layout. Do not invent extra crates. Do not put secrets or `.db` files in git.

```
LICENSE
NOTICE
README.md
CONTEXT.md
Cargo.toml                          # workspace
.gitignore
crates/dgw-engine/Cargo.toml
crates/dgw-engine/src/lib.rs
crates/dgw-engine/src/placeholder.rs
crates/dgw-engine/src/creator.rs
crates/dgw-engine/src/rules.rs
crates/dgw-engine/src/builtin.rs
crates/dgw-engine/src/walk.rs
crates/dgw-engine/src/sliding.rs
crates/dgw-engine/src/mapping.rs
crates/dgw-store/Cargo.toml
crates/dgw-store/src/lib.rs
crates/dgw-store/src/crypto.rs
crates/dgw-store/src/sqlite.rs
crates/dgw/Cargo.toml
crates/dgw/src/main.rs
crates/dgw/src/lib.rs
crates/dgw/src/config.rs
crates/dgw/src/logging.rs
crates/dgw/src/process.rs
crates/dgw/src/proxy/mod.rs
crates/dgw/src/proxy/forward.rs
crates/dgw/src/proxy/sse.rs
crates/dgw/src/admin/mod.rs
crates/dgw/src/admin/api.rs
crates/dgw/src/admin/auth.rs
ui/package.json
ui/vite.config.ts
ui/tsconfig.json
ui/index.html
ui/src/main.tsx
ui/src/App.tsx
ui/src/api.ts
ui/src/pages/Status.tsx
ui/src/pages/Rules.tsx
ui/src/pages/Upstream.tsx
ui/src/pages/Keys.tsx
ui/src/pages/Traffic.tsx
plugin/claude-code/plugin.json
plugin/claude-code/README.md
plugin/claude-code/skills/dgw/SKILL.md
plugin/claude-code/hooks/hooks.json
plugin/claude-code/scripts/dgw.mjs
.github/workflows/ci.yml
.github/workflows/release.yml
docs/adr/*.md                       # already present
docs/superpowers/plans/             # this plan
```

**Responsibility**

| Path | Responsibility |
|---|---|
| `crates/dgw-engine` | Placeholder syntax, Creator, rules, JSON walk, sliding-window restore, `MappingStore` trait. No HTTP, no SQLite. |
| `crates/dgw-store` | HKDF/AES-GCM/HMAC + SQLite implementation of `MappingStore`. |
| `crates/dgw` | Binary, config.toml, process/pid, proxy, admin API, embed UI, logging. |
| `ui/` | Management SPA, built into the binary. |
| `plugin/claude-code/` | Claude Code plugin: start/stop, Base URL chaining, optional GitHub download with SHA-256. |

---

## Locked behavior (do not re-litigate)

- Protocols: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses; same-prefix passthrough; multipart rejected; `count_tokens` is in-scope.
- Do not add headers/query/path for tenancy. Creator = SHA-256 of `x-api-key` else `Authorization` else `api-key`, else `anonymous`.
- Restore only when Creator matches. Miss/expired/other-creator â†?emit token, `restore_miss`, do not abort. Abort only if the store cannot be queried and a valid placeholder appears.
- Scan only JSON/text string **values**. Headers, query, path, keys untouched except forcing upstream `Accept-Encoding: identity` on scannable requests.
- Fail-closed A1. Request body default 32 MiB, hard cap 128 MiB â†?413.
- Sliding-window SSE restore. Placeholder `{{TYPE_ULID}}` with Crockford ULID (26).
- Desktop auto-generates Master Key file; `DGW_MASTER_KEY` wins; Server Mode refuses Proxy Port without a key.
- Data dir: Windows `%LOCALAPPDATA%\dgw\`, macOS Application Support, Linux XDG. Ports 18787/18788. Idempotent start.
- Logs never contain bodies, plaintext, full placeholders, credentials, or the Master Key. No outbound telemetry.
- License Apache-2.0. Product name Desensitization Gateway. Binary/plugin id `dgw`.

---

### Task 1: Repository skeleton and Apache-2.0

**Files:**
- Create: `LICENSE`, `NOTICE`, `.gitignore`, `Cargo.toml`, `crates/dgw-engine/Cargo.toml`, `crates/dgw-engine/src/lib.rs`, `README.md`
- Keep: `CONTEXT.md`, `docs/adr/*`

- [ ] **Step 1: Initialize git if missing**

Run:

```powershell
git init
git add CONTEXT.md docs
git commit -m "docs: capture domain glossary and ADRs"
```

Expected: repo exists, docs committed.

- [ ] **Step 2: Write `.gitignore`**

```
/target
/ui/node_modules
/ui/dist
**/*.db
**/*.db-wal
**/*.db-shm
.env
master.key
*.exe
.worktrees
worktrees
```

- [ ] **Step 3: Write workspace `Cargo.toml` and engine crate that compiles**

Root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/dgw-engine"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"

[workspace.dependencies]
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ulid = { version = "1", features = ["serde"] }
regex = "1"
once_cell = "1"
sha2 = "0.10"
hex = "0.4"
```

Do not put a GitHub repository URL in Cargo.toml until the origin exists. Plugin download tests inject `repo` as a function argument; production `plugin.json` is filled in Task 16 with the real `owner/name` of this repo.

`crates/dgw-engine/Cargo.toml`:

```toml
[package]
name = "dgw-engine"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
ulid.workspace = true
regex.workspace = true
once_cell.workspace = true
sha2.workspace = true
hex.workspace = true
```

`crates/dgw-engine/src/lib.rs`:

```rust
pub fn crate_name() -> &'static str {
    "dgw-engine"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "dgw-engine");
    }
}
```

Copy the official Apache-2.0 license text into `LICENSE`. `NOTICE`:

```
Desensitization Gateway
Copyright 2026 Desensitization Gateway contributors

Licensed under the Apache License, Version 2.0.
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p dgw-engine`
Expected: PASS (`crate_name_is_stable`)

- [ ] **Step 5: Commit**

```powershell
git add LICENSE NOTICE .gitignore Cargo.toml crates/dgw-engine README.md
git commit -m "chore: Apache-2.0 workspace skeleton"
```

---

### Task 2: Placeholder syntax (TYPE + ULID)

**Files:**
- Create: `crates/dgw-engine/src/placeholder.rs`
- Modify: `crates/dgw-engine/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use veil_engine::placeholder::{parse_placeholder, Placeholder, PLACEHOLDER_REGEX};

#[test]
fn formats_and_parses_roundtrip() {
    let p = Placeholder::new("PHONE", "01JQC3K7N8R2T4V6W8X0Y2Z4".parse().unwrap());
    let s = p.format();
    assert_eq!(s, "{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4}}");
    let back = parse_placeholder(&s).unwrap();
    assert_eq!(back.type_prefix, "PHONE");
    assert_eq!(back.format(), s);
}

#[test]
fn rejects_docs_examples_and_mustache() {
    assert!(parse_placeholder("{{PHONE_xxxx}}").is_none());
    assert!(parse_placeholder("{{TODO_1}}").is_none());
    assert!(parse_placeholder("{{phone_01JQC3K7N8R2T4V6W8X0Y2Z4}}").is_none());
}

#[test]
fn regex_does_not_match_inside_longer_braces_partial() {
    let text = "see {{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4}} please";
    let caps: Vec<_> = PLACEHOLDER_REGEX.find_iter(text).map(|m| m.as_str()).collect();
    assert_eq!(caps, vec!["{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4}}"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dgw-engine -- placeholder`
Expected: FAIL compiling (`placeholder` module missing)

- [ ] **Step 3: Implement**

`placeholder.rs`:

```rust
use once_cell::sync::Lazy;
use regex::Regex;
use ulid::Ulid;

/// `{{TYPE_ULID}}` where TYPE is [A-Z][A-Z0-9]{0,31} and ULID is 26 Crockford chars.
pub static PLACEHOLDER_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\{\{([A-Z][A-Z0-9]{0,31})_([0-9A-HJKMNP-TV-Z]{26})\}\}").unwrap()
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    pub type_prefix: String,
    pub id: Ulid,
}

impl Placeholder {
    pub fn new(type_prefix: impl Into<String>, id: Ulid) -> Self {
        Self { type_prefix: type_prefix.into(), id }
    }

    pub fn fresh(type_prefix: impl Into<String>) -> Self {
        Self::new(type_prefix, Ulid::new())
    }

    pub fn format(&self) -> String {
        format!("{{{{{}_{}}}}}", self.type_prefix, self.id.to_string())
    }
}

pub fn parse_placeholder(token: &str) -> Option<Placeholder> {
    let caps = PLACEHOLDER_REGEX.captures(token)?;
    if caps.get(0)?.as_str() != token {
        return None;
    }
    let type_prefix = caps.get(1)?.as_str().to_string();
    let id = Ulid::from_string(caps.get(2)?.as_str()).ok()?;
    Some(Placeholder { type_prefix, id })
}
```

Export from `lib.rs`: `pub mod placeholder;`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dgw-engine placeholder`
Expected: PASS

- [ ] **Step 5: Commit**

```powershell
git add crates/dgw-engine
git commit -m "feat: parse and format TYPE_ULID placeholders"
```

---

### Task 3: Creator derivation from existing headers

**Files:**
- Create: `crates/dgw-engine/src/creator.rs`
- Modify: `crates/dgw-engine/src/lib.rs`, `crates/dgw-engine/Cargo.toml` (add `http` crate)

Add workspace dep: `http = "1"`. Engine depends on `http`.

- [ ] **Step 1: Write the failing tests**

```rust
use veil_engine::creator::{creator_from_headers, ANONYMOUS, Creator};
use http::{HeaderMap, HeaderValue};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        h.append(http::HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
    }
    h
}

#[test]
fn prefers_x_api_key_over_authorization() {
    let a = creator_from_headers(&headers(&[
        ("x-api-key", "sk-ant-aaa"),
        ("authorization", "Bearer tok"),
    ]));
    let b = creator_from_headers(&headers(&[("x-api-key", "sk-ant-aaa")]));
    assert_eq!(a, b);
    let c = creator_from_headers(&headers(&[("authorization", "Bearer tok")]));
    assert_ne!(a, c);
}

#[test]
fn bearer_is_case_insensitive_and_trimmed() {
    let a = creator_from_headers(&headers(&[("authorization", "Bearer abc")]));
    let b = creator_from_headers(&headers(&[("Authorization", "  bearer abc  ")]));
    assert_eq!(a, b);
}

#[test]
fn no_credentials_is_anonymous() {
    assert_eq!(creator_from_headers(&HeaderMap::new()), ANONYMOUS);
}

#[test]
fn ignores_cookies() {
    let a = creator_from_headers(&headers(&[("cookie", "session=secret")]));
    assert_eq!(a, ANONYMOUS);
}

#[test]
fn display_is_hex_prefix_only() {
    let c = creator_from_headers(&headers(&[("x-api-key", "sk")]));
    let s = c.prefix8();
    assert_eq!(s.len(), 8);
    assert!(!s.contains("sk"));
}
```

- [ ] **Step 2: Run tests â€?expect FAIL (module missing)**

Run: `cargo test -p dgw-engine creator`

- [ ] **Step 3: Implement**

```rust
use http::HeaderMap;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Creator(pub [u8; 32]);

pub const ANONYMOUS: Creator = Creator([0u8; 32]); // overwritten in OnceLock below is NOT allowed; compute once:

impl Creator {
    pub fn from_secret(s: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(s.as_bytes());
        let out = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&out);
        Creator(id)
    }

    pub fn prefix8(&self) -> String {
        hex::encode(&self.0[..4])
    }

    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

fn normalize_bearer(value: &str) -> String {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")) {
        format!("Bearer {}", rest.trim())
    } else {
        v.to_string()
    }
}

pub fn creator_from_headers(headers: &HeaderMap) -> Creator {
    if let Some(v) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Creator::from_secret(t);
        }
    }
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let n = normalize_bearer(v);
        if !n.is_empty() {
            return Creator::from_secret(&n);
        }
    }
    if let Some(v) = headers.get("api-key").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Creator::from_secret(t);
        }
    }
    Creator::from_secret("dgw:anonymous")
}

pub fn anonymous_creator() -> Creator {
    Creator::from_secret("dgw:anonymous")
}

pub const ANONYMOUS_LABEL: &str = "anonymous";
```

Fix the tests to compare against `anonymous_creator()` instead of a zero array. Export `anonymous_creator` as the canonical empty-cred identity. Put this in the test file:

```rust
assert_eq!(creator_from_headers(&HeaderMap::new()), anonymous_creator());
```

- [ ] **Step 4: `cargo test -p dgw-engine creator` â€?PASS**

- [ ] **Step 5: Commit** `feat: derive Creator from existing auth headers`

---

### Task 4: Rule engine with priority, allowlist, non-overlapping spans

**Files:**
- Create: `crates/dgw-engine/src/rules.rs`
- Modify: `lib.rs`; add `fancy-regex` **or** use `regex` with a per-rule timeout via `std::thread` + `mpsc` (v1 requirement: custom regex timeout). Use a worker thread with `recv_timeout` so a hung regex does not block the process. Default timeout 50ms.

- [ ] **Step 1: Failing tests**

```rust
use veil_engine::rules::{Allowlist, Matcher, Rule, RuleSet};

fn ruleset(rules: Vec<Rule>) -> RuleSet {
    RuleSet::new(rules, Allowlist::from(["127.0.0.1", "localhost", "0.0.0.0", "::1"]))
}

#[test]
fn higher_priority_wins_and_consumes_span() {
    let conn = Rule::regex("conn", "CONNSTR", r"postgres://\\S+", 100);
    let ip = Rule::regex("ip", "IP", r"\\d+\\.\\d+\\.\\d+\\.\\d+", 10);
    let set = ruleset(vec![conn, ip]);
    let hits = set.find_hits("postgres://u:p@10.0.0.5/db");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].type_prefix, "CONNSTR");
    assert_eq!(hits[0].plaintext, "postgres://u:p@10.0.0.5/db");
}

#[test]
fn allowlist_skips_exact_string() {
    let ip = Rule::regex("ip", "IP", r"127\\.0\\.0\\.1", 10);
    let set = ruleset(vec![ip]);
    assert!(set.find_hits("talk to 127.0.0.1 please").is_empty());
}

#[test]
fn disabled_rules_do_not_match() {
    let mut r = Rule::regex("phone", "PHONE", r"1[3-9]\\d{9}", 50);
    r.enabled = false;
    let set = ruleset(vec![r]);
    assert!(set.find_hits("call 13800138000").is_empty());
}

#[test]
fn dictionary_matcher_is_literal() {
    let r = Rule::dictionary("words", "SECRET", vec!["AKIAEXAMPLE".into()], 80);
    let set = ruleset(vec![r]);
    assert_eq!(set.find_hits("AKIAEXAMPLE in text")[0].plaintext, "AKIAEXAMPLE");
}

#[test]
fn regex_timeout_counts_as_miss_for_that_rule_only() {
    let evil = Rule::regex("evil", "X", r"(a+)+$", 1).with_timeout_ms(5);
    let phone = Rule::regex("phone", "PHONE", r"1[3-9]\\d{9}", 50);
    let set = ruleset(vec![evil, phone]);
    let hits = set.find_hits(&format!("{}13800138000", "a".repeat(20)));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].type_prefix, "PHONE");
}
```

(If the timeout test is flaky on the evil regex, substitute a matcher that sleeps in tests via `Matcher::FailOpenTimeout` test hook â€?production still uses thread timeout. Prefer a deterministic test hook: `#[cfg(test)] Matcher::BlockUntilTimeout`.)

- [ ] **Step 2: Run â€?FAIL missing module**

- [ ] **Step 3: Implement `Rule`, `RuleSet::find_hits(&self, text: &str) -> Vec<Hit>`**

Required behavior:
- Sort by priority descending, then stable id.
- Search remaining gaps only (interval tree or sorted occupied spans).
- Exact allowlist contains â†?skip that exact substring occurrence.
- On regex timeout: skip that rule, continue others.
- Never emit a hit whose text already contains `{{` from a previous replacement (pre-replace; spans are on original text).

- [ ] **Step 4: `cargo test -p dgw-engine rules` PASS**

- [ ] **Step 5: Commit** `feat: prioritized non-overlapping rule matching`

---

### Task 5: Built-in rule pack

**Files:**
- Create: `crates/dgw-engine/src/builtin.rs`
- Create: `crates/dgw-engine/src/ip.rs`
- Modify: `lib.rs`

- [ ] **Step 1: Failing tests**

```rust
use veil_engine::builtin::builtin_ruleset;
use std::net::IpAddr;

#[test]
fn parses_public_and_private_ips() {
    let set = builtin_ruleset();
    let hits = set.find_hits("edge 8.8.8.8 and dc 10.0.0.5");
    let values: Vec<_> = hits.iter().filter(|h| h.type_prefix == "IP").map(|h| h.plaintext.as_str()).collect();
    assert!(values.contains(&"8.8.8.8"));
    assert!(values.contains(&"10.0.0.5"));
}

#[test]
fn loopback_allowlisted() {
    let set = builtin_ruleset();
    assert!(set.find_hits("http://127.0.0.1:18787").iter().all(|h| h.type_prefix != "IP"));
    assert!(set.find_hits("listen on ::1").iter().all(|h| h.plaintext != "::1"));
}

#[test]
fn china_mobile_and_id() {
    let set = builtin_ruleset();
    let t = "mobile 13800138000 id 110101199003078515";
    let types: Vec<_> = set.find_hits(t).iter().map(|h| h.type_prefix.as_str()).collect();
    assert!(types.contains(&"PHONE"));
    assert!(types.contains(&"IDCARD"));
}

#[test]
fn connection_string_beats_inner_ip() {
    let set = builtin_ruleset();
    let hits = set.find_hits("postgres://u:s3cret@10.1.2.3:5432/app");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].type_prefix, "CONNSTR");
}

#[test]
fn akia_and_pem_header() {
    let set = builtin_ruleset();
    let hits = set.find_hits("AKIAIOSFODNN7EXAMPLE and -----BEGIN RSA PRIVATE KEY-----");
    let types: Vec<_> = hits.iter().map(|h| h.type_prefix.as_str()).collect();
    assert!(types.contains(&"AKSK"));
}

#[test]
fn email_builtin_disabled() {
    let set = builtin_ruleset();
    assert!(set.find_hits("a@example.com").iter().all(|h| h.type_prefix != "EMAIL"));
}

#[test]
fn garbage_is_not_an_ip() {
    assert!("not-an-ip".parse::<IpAddr>().is_err());
    let set = builtin_ruleset();
    assert!(set.find_hits("commit deadbeef").iter().all(|h| h.type_prefix != "IP"));
}
```

- [ ] **Step 2: Run â€?FAIL**

- [ ] **Step 3: Implement built-ins**

Priority order (high first):
1. PEM / AKIA / `sk-ant-` / `sk-` high-entropy / `Bearer eyJ` â†?`AKSK` (200)
2. Connection URI and ADO-style `Pwd=` strings â†?`CONNSTR` (180)
3. China ID (17 digits + check) â†?`IDCARD` (150)
4. China mobile â†?`PHONE` (140)
5. IP tokenizer: split on whitespace and common delimiters, try `IpAddr::parse` on tokens and on bracketed v6; skip allowlist; type `IP` (120)
6. Email regex, `enabled: false` (80)

IP matching **must parse**, not only regex dotted quads, so IPv6 works. Do not redact non-parseable tokens.

- [ ] **Step 4: `cargo test -p dgw-engine builtin` PASS**

- [ ] **Step 5: Commit** `feat: builtin secret, connstr, phone, id, and all-parseable IP rules`

---

### Task 6: JSON string-value walk (codec round-trip)

**Files:**
- Create: `crates/dgw-engine/src/walk.rs`
- Modify: `lib.rs`

- [ ] **Step 1: Failing tests**

```rust
use veil_engine::walk::{desensitize_json, restore_json};
use veil_engine::mapping::MemoryStore;
use veil_engine::creator::anonymous_creator;
use veil_engine::builtin::builtin_ruleset;
use serde_json::json;

#[test]
fn replaces_only_string_values_not_keys() {
    let store = MemoryStore::new();
    let rules = builtin_ruleset();
    let c = anonymous_creator();
    let input = json!({"13800138000": "13800138000"});
    let out = desensitize_json(&input, &rules, &store, &c).unwrap();
    assert!(out.as_object().unwrap().contains_key("13800138000"));
    let v = out["13800138000"].as_str().unwrap();
    assert!(v.starts_with("{{PHONE_"));
    assert!(!v.contains("13800138000"));
}

#[test]
fn nested_and_array_strings() {
    let store = MemoryStore::new();
    let out = desensitize_json(&json!({"messages":[{"content":"ip 8.8.8.8"}]}), &builtin_ruleset(), &store, &anonymous_creator()).unwrap();
    let s = out["messages"][0]["content"].as_str().unwrap();
    assert!(s.contains("{{IP_"));
    assert!(!s.contains("8.8.8.8"));
}

#[test]
fn password_with_quotes_survives_roundtrip() {
    let store = MemoryStore::new();
    let c = anonymous_creator();
    let secret = r#"postgres://u:p"a\b@10.1.2.3/db"#;
    let input = json!({"s": secret});
    let redacted = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
    let restored = restore_json(&redacted, &store, &c).unwrap();
    assert_eq!(restored["s"].as_str().unwrap(), secret);
}

#[test]
fn does_not_rescan_restored_text() {
    let store = MemoryStore::new();
    let c = anonymous_creator();
    let input = json!({"s": "13800138000"});
    let redacted = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
    let restored = restore_json(&redacted, &store, &c).unwrap();
    assert_eq!(restored["s"], "13800138000");
}

#[test]
fn same_creator_reuses_placeholder() {
    let store = MemoryStore::new();
    let c = anonymous_creator();
    let a = desensitize_json(&json!("13800138000"), &builtin_ruleset(), &store, &c).unwrap();
    let b = desensitize_json(&json!("13800138000"), &builtin_ruleset(), &store, &c).unwrap();
    assert_eq!(a, b);
}
```

`MemoryStore` is introduced here as an in-memory `MappingStore` in `mapping.rs` if Task 7 is not yet done â€?implement a minimal in-memory store in this task (see Step 3) so walk tests run.

- [ ] **Step 2: Run â€?FAIL**

- [ ] **Step 3: Implement walk + MemoryStore**

`MappingStore` trait in `mapping.rs`:

```rust
use crate::creator::Creator;
use crate::placeholder::Placeholder;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("unavailable")]
    Unavailable,
    #[error("internal: {0}")]
    Internal(String),
}

pub enum Lookup {
    Hit(String),
    Miss,
}

pub trait MappingStore {
    fn get_or_insert(&self, creator: &Creator, type_prefix: &str, plaintext: &str) -> Result<Placeholder, StoreError>;
    fn lookup(&self, creator: &Creator, placeholder: &Placeholder) -> Result<Lookup, StoreError>;
}
```

Walk algorithm:
- `desensitize_json` recursively clones JSON. On `Value::String`, run `find_hits`, replace from right to left so offsets stay valid, `get_or_insert` each hit. Numbers/bools/null/object keys unchanged.
- If any `get_or_insert` fails after a hit was found, return `Err(WalkError::MappingWrite)` (fail-closed A1).
- `restore_json` finds `PLACEHOLDER_REGEX` in strings; `lookup` Hit â†?substitute plaintext (do not scan again); Miss â†?leave token; `Unavailable` â†?`Err(WalkError::StoreUnavailable)`.

- [ ] **Step 4: `cargo test -p dgw-engine walk` PASS**

- [ ] **Step 5: Commit** `feat: JSON string-value desensitize and restore`

---

### Task 7: Sliding-window restore

**Files:**
- Create: `crates/dgw-engine/src/sliding.rs`
- Modify: `lib.rs`

- [ ] **Step 1: Failing tests**

```rust
use veil_engine::sliding::RestoreWindow;
use veil_engine::mapping::MemoryStore;
use veil_engine::placeholder::Placeholder;
use veil_engine::creator::anonymous_creator;
use ulid::Ulid;

#[test]
fn restores_placeholder_split_across_pushes() {
    let store = MemoryStore::new();
    let c = anonymous_creator();
    let p = store.get_or_insert(&c, "PHONE", "13800138000").unwrap();
    let token = p.format();
    let mid = token.len() / 2;
    let mut w = RestoreWindow::new(&store, c);
    let mut out = String::new();
    out.push_str(&w.push(&token[..mid]).unwrap());
    out.push_str(&w.push(&token[mid..]).unwrap());
    out.push_str(&w.flush().unwrap());
    assert_eq!(out, "13800138000");
}

#[test]
fn emits_safe_prefix_immediately() {
    let store = MemoryStore::new();
    let mut w = RestoreWindow::new(&store, anonymous_creator());
    let out = w.push("hello ").unwrap();
    assert_eq!(out, "hello ");
}

#[test]
fn unknown_valid_placeholder_emitted_unchanged() {
    let store = MemoryStore::new();
    let fake = Placeholder::new("PHONE", Ulid::new()).format();
    let mut w = RestoreWindow::new(&store, anonymous_creator());
    let mut out = w.push(&fake).unwrap();
    out.push_str(&w.flush().unwrap());
    assert_eq!(out, fake);
}

#[test]
fn store_unavailable_with_valid_token_is_error() {
    let store = veil_engine::mapping::DownStore;
    let fake = Placeholder::new("PHONE", Ulid::new()).format();
    let mut w = RestoreWindow::new(&store, anonymous_creator());
    let err = w.push(&fake).unwrap_err();
    assert!(matches!(err, veil_engine::walk::WalkError::StoreUnavailable));
}
```

- [ ] **Step 2: Run â€?FAIL**

- [ ] **Step 3: Implement `RestoreWindow`**

Algorithm:
- Hold a tail buffer. Max tail = max placeholder length (2 + 32 + 1 + 26 + 2 = 63, use 96).
- On `push(s)`: `buf = tail + s`. Find all complete placeholders with regex. Emit decoded replacements for complete matches. Keep only a suffix that is a prefix of `{{...}}` (scan backward for `{`). Anything before that suffix is safe: emit as-is.
- `flush` emits remaining tail (or restores if a complete token now sits there).
- Bound the buffer; if tail would exceed 96 without a leading `{{`, emit it (cannot be a placeholder).

SSE framing is **not** this type's job. This type restores a Unicode string (already JSON-decoded or raw text). The proxy will apply it inside JSON string values / SSE data payloads.

- [ ] **Step 4: PASS** `cargo test -p dgw-engine sliding`

- [ ] **Step 5: Commit** `feat: sliding-window placeholder restore`

---

### Task 8: Encrypted SQLite MappingStore

**Files:**
- Create: `crates/dgw-store/Cargo.toml`, `crates/dgw-store/src/lib.rs`, `crypto.rs`, `sqlite.rs`
- Modify: root `Cargo.toml` members

Workspace deps to add: `aes-gcm`, `hkdf`, `rand`, `rusqlite` with `bundled`, `hmac`, `zeroize`, `parking_lot`, `tempfile` (dev).

- [ ] **Step 1: Failing tests in `crates/dgw-store/src/crypto.rs` and `sqlite.rs`**

```rust
#[test]
fn roundtrip_cell() {
    let mk = [7u8; 32];
    let keys = derive_keys(&mk);
    let ct = encrypt(&keys.enc, b"13800138000");
    assert_ne!(ct, b"13800138000");
    assert_eq!(decrypt(&keys.enc, &ct).unwrap(), b"13800138000");
}

#[test]
fn hmac_index_needs_mac_key() {
    let a = derive_keys(&[1u8; 32]);
    let b = derive_keys(&[2u8; 32]);
    let p = b"13800138000";
    assert_ne!(lookup_hmac(&a.mac, &anonymous_creator(), p), lookup_hmac(&b.mac, &anonymous_creator(), p));
}

#[test]
fn sqlite_get_or_insert_stable_and_creator_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("m.db"), &[9u8; 32]).unwrap();
    let c1 = Creator::from_secret("k1");
    let c2 = Creator::from_secret("k2");
    let p1 = store.get_or_insert(&c1, "PHONE", "13800138000").unwrap();
    let p1b = store.get_or_insert(&c1, "PHONE", "13800138000").unwrap();
    assert_eq!(p1, p1b);
    let p2 = store.get_or_insert(&c2, "PHONE", "13800138000").unwrap();
    assert_ne!(p1, p2);
    assert!(matches!(store.lookup(&c2, &p1).unwrap(), Lookup::Miss));
    assert!(matches!(store.lookup(&c1, &p1).unwrap(), Lookup::Hit(s) if s == "13800138000"));
}

#[test]
fn ttl_and_lru_cap() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("m.db"), &[9u8; 32]).unwrap()
        .with_ttl(Duration::from_secs(0)) // expire immediately on next purge
        .with_cap(2);
    let c = anonymous_creator();
    store.get_or_insert(&c, "PHONE", "1").unwrap();
    store.purge().unwrap();
    let p = Placeholder::new("PHONE", /* id from first insert - fetch via get_or_insert again */);
    // After purge, a new insert for same plaintext may mint a new ULID.
    let p_new = store.get_or_insert(&c, "PHONE", "1").unwrap();
    let _ = p_new;
}
```

Write the TTL test as: insert; `purge` with ttl=0; `lookup` of the old placeholder returns `Miss`.

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement**

Schema:

```sql
CREATE TABLE mappings (
  placeholder_id TEXT PRIMARY KEY,
  type_prefix TEXT NOT NULL,
  creator_hex TEXT NOT NULL,
  plaintext_hmac BLOB NOT NULL,
  plaintext_ct BLOB NOT NULL,
  last_access_unix INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_creator_hmac ON mappings(creator_hex, plaintext_hmac);
CREATE INDEX idx_creator_lru ON mappings(creator_hex, last_access_unix);
```

Crypto: HKDF-SHA256, info `dgw-mapping-v1-enc` / `dgw-mapping-v1-mac`, salt `dgw-mapping-v1`. AES-256-GCM, 12-byte random nonce prefixed to ciphertext. HMAC-SHA256(mac, creator.bytes || plaintext).

Defaults: TTL 90 days, cap 100_000 per creator, LRU delete on overflow. `purge` also on a timer in the binary (Task 11), but unit-testable here.

- [ ] **Step 4: `cargo test -p dgw-store` PASS**

- [ ] **Step 5: Commit** `feat: encrypted SQLite mapping store`

---

### Task 9: Config.toml, data dir, master key bootstrap

**Files:**
- Create: `crates/dgw/Cargo.toml`, `src/lib.rs`, `src/config.rs`, `src/data_dir.rs`, `src/master_key.rs`, `src/main.rs`
- Modify: workspace members += `crates/dgw`

- [ ] **Step 1: Failing tests** (`crates/dgw/src/config.rs` cfg tests using tempfile)

Cases:
- `default_data_dir()` on Windows ends with `\\dgw` and uses local app data (mock via `DGW_DATA_DIR` in tests â€?tests must set `DGW_DATA_DIR` and assert override wins).
- Missing key file + no env in `Mode::Desktop` â†?writes 32 random bytes hex to `master.key` (mode 0600 when OS supports).
- `Mode::Server` + missing key + missing env â†?`Err(NoMasterKey)`.
- `DGW_MASTER_KEY` overrides file.
- Roundtrip `Config` serde toml: ports 18787/18788, three upstreams defaulting to `https://api.anthropic.com` and `https://api.openai.com`, ttl 90 days, body 32 MiB, bind `127.0.0.1`.

Default config struct (keep names stable):

```rust
pub struct Config {
    pub bind: String,            // 127.0.0.1
    pub proxy_port: u16,         // 18787
    pub management_port: u16,    // 18788
    pub mode: Mode,              // Desktop | Server
    pub anthropic_upstream: String,
    pub openai_completions_upstream: String,
    pub openai_responses_upstream: String,
    pub request_body_limit_mib: u64, // 32, clamp to 128
    pub mapping_ttl_days: u64,       // 90
    pub mapping_cap_per_creator: u64, // 100000
    pub admin_token_hash: String,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    pub saved_client_anthropic_base_url: Option<String>,
    pub rules: Vec<RuleConfig>,
    pub allowlist: Vec<String>,
}
```

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement load/save of `config.toml`, key file, env overrides `DGW_DATA_DIR`, `DGW_MASTER_KEY`, `DGW_BIND`, `DGW_NO_DOWNLOAD`, `DGW_MODE=server|desktop`. Generate admin token on first desktop start, store SHA-256 in config, write once to `admin.token` file for the UI to read locally (never returned by API after).

- [ ] **Step 4: `cargo test -p dgw config` PASS**

- [ ] **Step 5: Commit** `feat: config.toml, data dir, and master key bootstrap`

---

### Task 10: Process pid, idempotent start, logging red lines

**Files:**
- Create: `crates/dgw/src/process.rs`, `crates/dgw/src/logging.rs`

- [ ] **Step 1: Failing tests**

- `write_pid` / `read_pid` / `is_self_running` using a temp data dir.
- `try_acquire` when pid live â†?`AlreadyRunning { proxy, management }` success-equivalent for CLI start.
- `stop` only kills recorded pid, not a random pid.
- Logger formatter: a record with a field named `body` or a message containing `Authorization: Bearer` is still formatted, but helper `assert_no_secret(event, secret)` used in a test where we log `hit_types=["PHONE"]` and the plaintext is NOT in the output.

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement JSON logs to `data_dir/logs/dgw.log` with rotation (5 MiB Ã— 3). Fields allowed: ts, level, protocol, method, path_template, status, upstream_ms, streaming, hit_types, hit_counts, creator_prefix8, error_class. Never log headers, bodies, tokens.

CLI in `main.rs` (even if proxy is still a stub):

```
dgw start | stop | status | ui
```

`start` daemonizes on Unix; on Windows starts a detached child if not already in service mode, or runs foreground with `--foreground` (plugin uses `--foreground` only when it owns the job). Default `dgw start` on Windows: spawn `CREATE_NEW_PROCESS_GROUP` detached, write pid.

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat: pid-idempotent process control and redacted logs`

---

### Task 11: Reverse proxy core (size limit, identity encoding, fail-closed)

**Files:**
- Create: `crates/dgw/src/proxy/mod.rs`, `forward.rs`, `classify.rs`
- Dev-dep: `tokio`, `axum`, `reqwest`, `tower-http`, `http-body-util`, `wiremock` or a local `hyper` test server.

- [ ] **Step 1: Failing integration tests** `crates/dgw/tests/proxy_core.rs`

Use axum test + a mock upstream:

1. `GET /v1/models` empty body â†?forwarded, no mapping writes.
2. JSON POST with no hits â†?forwarded, body equal.
3. JSON POST with phone â†?upstream body contains `{{PHONE_` and not the digits; mapping row exists.
4. Body 32 MiB+1 â†?413, mock upstream receives 0 requests.
5. Outgoing request to mock has no `accept-encoding` gzip (identity).
6. Multipart content-type â†?415, not forwarded.
7. Unknown path `/secret` â†?404, not forwarded.
8. Store forced unavailable + body with phone â†?502, not forwarded.
9. Store unavailable + GET empty â†?still forwarded.

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement classify + forward**

Path classification:

| Prefix | Family | Upstream config field |
|---|---|---|
| `/v1/messages` | Anthropic | `anthropic_upstream` |
| `/v1/chat/completions` | OpenAI Completions | `openai_completions_upstream` |
| `/v1/responses` | OpenAI Responses | `openai_responses_upstream` |
| `/v1/models` | If request also has anthropic headers â†?Anthropic, else OpenAI Completions | same |

Simpler v1: `/v1/messages`* â†?Anthropic; `/v1/chat/completions`* and `/v1/models`* and `/v1/embeddings`* â†?OpenAI Completions; `/v1/responses`* â†?OpenAI Responses.

Forward:
- Copy method, path, query, all headers except `host` and `accept-encoding` (set identity when body will be scanned or response restored â€?i.e. all classified routes).
- Rebuild Host from upstream URL.
- Scan JSON if content-type json or body starts with `{`/`[`.
- Non-JSON text: scan as one string.
- Empty body: skip scan.

- [ ] **Step 4: `cargo test -p dgw --test proxy_core` PASS**

- [ ] **Step 5: Commit** `feat: proxy classify, size limit, and fail-closed forward`

---

### Task 12: SSE sliding restore for Anthropic and OpenAI

**Files:**
- Create: `crates/dgw/src/proxy/sse.rs`
- Modify: `forward.rs`
- Test: `crates/dgw/tests/sse_restore.rs`

- [ ] **Step 1: Failing tests**

Feed a mock upstream SSE:

Anthropic:

```
event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"call {{PHONE_

event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"01JQC...}} now"}}
```

(Use a real ULID from a mapping inserted before the request.)

Assert the client-side reconstructed text is `call 13800138000 now` and frames still have `event: content_block_delta`.

OpenAI Chat Completions: `data: {"choices":[{"delta":{"content":"{{IP_"}}]}` split across two chunks.

OpenAI Responses: `event: response.output_text.delta` with `delta` string split.

Non-stream JSON response with placeholder in `content` restores fully.

If store is down and a valid placeholder appears in the stream: connection ends with an SSE error event `event: error` / `data: {"error":{"type":"dgw_restore_failed"}}` and is not completed as 200 success with leftover tokens. (A1 store-unavailable path.)

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement**

Parse SSE as events (split on `\n\n`). For each `data:` line, if JSON, walk string values with `RestoreWindow` **per JSON string field** (keep a map of field-path â†?window, or one window per event string â€?v1: independently restore each complete JSON string value; if a JSON string itself is split across SSE events, keep a window keyed by the current text-delta path).

Minimum viable that still honors "split across chunks":

- Anthropic `delta.text` concatenates across events â†?one `RestoreWindow` for that content block.
- OpenAI `choices[].delta.content` â†?one window per choice index.
- OpenAI Responses `delta` text â†?one window.

Do not buffer the entire response. Emit SSE events as soon as the window produces bytes, re-encoded as JSON string fragments (codec round-trip). If the window cannot emit yet, hold the event.

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat: streaming SSE restore for three protocol families`

---

### Task 13: Management API

**Files:**
- Create: `crates/dgw/src/admin/mod.rs`, `api.rs`, `auth.rs`, `traffic.rs`
- Test: `crates/dgw/tests/admin_api.rs`

- [ ] **Step 1: Failing tests**

Auth: requests without admin token to mutating routes â†?401. GET `/api/status` on localhost may use token from header `x-dgw-admin-token` or cookie.

Never returns: mapping plaintext, master key, full admin token, full creator hashes (prefix8 only).

Endpoints:

```
GET  /api/status
GET  /api/rules
PUT  /api/rules                 # full replace + write config.toml
POST /api/rules/dry-run         # { "text": "..." } -> hits without storing
GET  /api/allowlist
PUT  /api/allowlist
GET  /api/upstream
PUT  /api/upstream
PUT  /api/settings              # ttl, body limit (clamped)
POST /api/master-key            # { "new_key": "hex" } rotate, no echo
POST /api/mappings/purge        # { "creator_prefix": "abcd1234" } optional
GET  /api/traffic               # SSE of recent events (path template, status, hit types)
POST /api/admin-token/reset
```

Dry-run uses the engine on the provided text only; does not call upstream; does not persist mappings (use a throwaway MemoryStore).

- [ ] **Step 2: FAIL**

- [ ] **Step 3: Implement Axum router on management port. Bind default 127.0.0.1. If bind is not loopback, require admin token on **all** routes including static UI.

Live traffic: an in-memory ring buffer (512 events) filled by the proxy; SSE endpoint ticks it. Event struct: `ts, method, path_template, status, streaming, hit_types, hit_counts, latency_ms, error_class, creator_prefix8`.

- [ ] **Step 4: PASS**

- [ ] **Step 5: Commit** `feat: management API with admin token and dry-run`

---

### Task 14: Embed React UI (five screens)

**Files:**
- Create: entire `ui/` as listed in File structure
- Modify: `crates/dgw/src/admin/mod.rs` to serve `rust-embed` of `ui/dist`
- Modify: `crates/dgw/build.rs` to run `npm ci && npm run build` when `ui/package.json` exists, skip if `DGW_SKIP_UI_BUILD=1`

- [ ] **Step 1: Write UI unit tests** (vitest) for:
- Rules dry-run renderer shows type + snippet **of the sample**, never calls a real network in unit tests (mock `api.ts`).
- Traffic table columns do not include a `query` or `body` field in the TypeScript type.

`ui/src/api.ts` types must not contain `plaintext` or `master_key`.

- [ ] **Step 2: Run `npm test` â€?FAIL until pages exist**

- [ ] **Step 3: Implement screens**

Use a distinctive, dense ops-console look (not a generic purple dashboard). Stack: Vite, React 18, TypeScript. No analytics.

Pages:
1. Status â€?process, ports, three upstreams, counts, last error class.
2. Rules â€?list, enable, priority, regex/dict editor, allowlist, dry-run pane.
3. Upstream & Access â€?three URLs, bind, optional TLS paths, body limit, TTL, copyable proxy Base URL.
4. Keys & Danger â€?master key status, rotate form, purge by creator prefix, reset admin token.
5. Live Traffic â€?streaming table from `GET /api/traffic`.

Login: if token required, a single token field stored in memory/sessionStorage, sent as `x-dgw-admin-token`.

- [ ] **Step 4: `npm test` PASS; `cargo test -p dgw --test admin_api` still PASS; manually `dgw start --foreground` then open `http://127.0.0.1:18788`**

- [ ] **Step 5: Commit** `feat: embedded management UI with five screens`

---

### Task 15: Claude Code plugin (start/stop, Base URL chaining, pinned download)

**Files:**
- Create: `plugin/claude-code/plugin.json`, `README.md`, `skills/dgw/SKILL.md`, `hooks/hooks.json`, `scripts/dgw.mjs`, `scripts/dgw.test.mjs`

- [ ] **Step 1: Write failing Node tests** (plain `node:test` in `scripts/dgw.test.mjs`)

Test the pure functions in `scripts/dgw.mjs` (export them):

1. `resolveBinary({ env, dataDir, pathExists })` order: `DGW_BIN` â†?`dataDir/bin/dgw(.exe)` â†?`PATH`.
2. `pinnedVersion()` reads `plugin.json` field `dgw.version` (e.g. `"0.1.0"`) not `latest`.
3. `downloadUrl(version, platform, arch)` is exactly `https://github.com/<org>/<repo>/releases/download/v{version}/dgw-{os}-{arch}[.exe]` and rejects hosts that are not `github.com`.
4. `verifySha256(fileBytes, sumsText, assetName)` true/false; mismatch returns false.
5. `chainBaseUrl({ current, proxyUrl })` : if current is empty or already proxyUrl, saved upstream unchanged; if current is some other http(s) URL, return `{ client: proxyUrl, saveUpstream: current }`.
6. `restoreBaseUrl({ saved, proxyUrl, current })` restores saved when current === proxyUrl.
7. `DGW_NO_DOWNLOAD=1` + missing binary â†?`{ ok:false, reason: "no_binary" }` without fetching.

Mock `fetch` in tests. Do **not** hit the network.

- [ ] **Step 2: `node --test plugin/claude-code/scripts/dgw.test.mjs` FAIL**

- [ ] **Step 3: Implement**

`plugin.json` (adjust to the current Claude Code plugin schema; required fields: name `dgw`, version, description, and a `dgw.version` pin matching the gateway release tag):

```json
{
  "name": "dgw",
  "version": "0.1.0",
  "description": "Desensitization Gateway for Claude Code",
  "dgw": { "version": "0.1.0", "repo": "ORG/DesensitizationGateway" }
}
```

`hooks/hooks.json`: SessionStart command runs `node scripts/dgw.mjs start`. Failure prints a warning and **does not** exit non-zero in a way that blocks the session (catch errors, exit 0, message: "dgw: gateway not running, traffic is not desensitized").

Slash commands via SKILL.md: `/dgw start|stop|status|ui`.

Settings writes:
- Read user `~/.claude/settings.json` (create if missing).
- Backup previous `env.ANTHROPIC_BASE_URL` (or equivalent Claude Code env map) into gateway config via `POST /api/upstream` only when chaining the first time.
- Set `ANTHROPIC_BASE_URL=http://127.0.0.1:18787`.
- Never write project `.claude/`.
- Do not touch OAuth files or API keys.

Download:
- Only when binary missing and `DGW_NO_DOWNLOAD` unset.
- Fetch `SHA256SUMS` and the pinned asset from the tag.
- Verify hash, chmod +x, then exec.
- Mismatch: delete file, do not exec.

- [ ] **Step 4: Tests PASS**

- [ ] **Step 5: Commit** `feat: Claude Code plugin with chained Base URL and hashed download`

---

### Task 16: CI, cross-compile, SHA256SUMS, integration smoke

**Files:**
- Create: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `crates/dgw/tests/smoke.rs`

- [ ] **Step 1: Write smoke test** (ignored if no network): start `dgw` on ephemeral ports with a mock upstream, POST a Messages-shaped JSON containing `13800138000`, assert mock received placeholder, client received restored JSON.

- [ ] **Step 2: Run `cargo test --workspace` â€?FAIL if smoke not wired; then implement any glue**

- [ ] **Step 3: CI workflow**

On PR: `cargo test --workspace`, `cargo fmt --check`, `npm test` in `ui/`, `node --test plugin/claude-code/scripts/dgw.test.mjs`.

Release workflow on tag `v*`:
- Build: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`
- Name assets: `dgw-windows-x64.exe`, `dgw-linux-x64`, `dgw-linux-arm64`, `dgw-macos-arm64`, `dgw-macos-x64`
- Generate `SHA256SUMS`
- Upload GitHub Release

README: Desktop vs Server, official subscription vs custom API, chaining, fail-closed, no telemetry, Apache-2.0.

- [ ] **Step 4: `cargo test --workspace` PASS on the implementation machine**

- [ ] **Step 5: Commit** `ci: test, release assets, and smoke path`

---

## Self-review

### Spec coverage

| Requirement | Task |
|---|---|
| Zero-intrusion Base URL | 11, 15 |
| Anthropic Messages + SSE | 11, 12 |
| OpenAI Chat Completions + Responses | 11, 12 |
| Official subscription + custom API chaining | 9, 15 |
| Outbound desensitize, plaintext never upstream | 4â€?, 11 |
| Inbound restore, streaming sliding window | 7, 12 |
| Custom rules UI + dry-run | 13, 14 |
| Built-in pack including all parseable IPs | 5 |
| Creator-bound restore, no extra headers | 3, 6, 8 |
| Encrypted SQLite + master key UI/env | 8, 9, 13 |
| Fail-closed A1 + 32 MiB cap | 11 |
| Split ports, admin token | 9, 13 |
| Claude Code plugin start/stop/download | 15 |
| Desktop + server same binary | 9, 10 |
| Apache-2.0, no telemetry | 1, 10, 16 |

### Placeholder scan

This plan has no TBD/TODO implementation holes in task steps. GitHub `ORG` in plugin download URL is filled at release with the real origin; tests inject it.

### Type consistency

Stable names used throughout: `Placeholder`, `Creator`, `MappingStore`, `Lookup::{Hit,Miss}`, `RestoreWindow`, `Config`, `anonymous_creator`, type prefixes `PHONE|IDCARD|IP|CONNSTR|AKSK|EMAIL`.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-10-desensitization-gateway.md`.

The operator already chose **Subagent-Driven** execution (fresh subagent per task, main agent reviews between tasks).

**REQUIRED SUB-SKILL when executing:** superpowers:subagent-driven-development.

Do not start Task 1 until the operator confirms "go" in this session (or clicks Approve). First execution step is still: init git if needed, then Task 1 in a worktree if the repo exists.




