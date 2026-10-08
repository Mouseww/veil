# Veil redesign: Claude risk-signal reduction

Status: draft for implementation. Marketing claim: lower geo/environment signals that correlate with Claude account risk. Do not promise "will not ban."

## Positioning

- Primary: local gateway that reduces China/geo fingerprints reaching Anthropic (and similar upstreams).
- Secondary: existing secret redaction (passwords, tokens, IPs).
- Honest scope: body + selected headers + egress IP via outbound proxy. Not absolute anonymity.

## MVP features

1. **Outbound proxy** — HTTP/HTTPS/SOCKS5 URL in config; reqwest Client uses it for upstream forwards only (not management UI).
2. **Rule packs** — group built-in rules: `secrets` (current), `region` (new, on by default for anti-ban narrative). UI toggles packs; rules remain individually overridable.
3. **Header rewrite** — configurable outbound header mutations. Default region profile:
   - Set or replace `Accept-Language` to a non-CN value (e.g. `en-US,en;q=0.9`)
   - Optionally strip/replace other locale headers if present
   - Keep auth headers untouched (`x-api-key`, `Authorization`, `api-key`, cookies)
4. **Passthrough** — unchanged: non-matching body text still forwards; fail-closed only when scan cannot prove cleanliness or mapping persist fails.

## Region pack (v1 matchers)

- Timezone IDs: `Asia/Shanghai`, `Asia/Chongqing`, `Asia/Urumqi`, `Asia/Harbin`, `PRC`, `CST` in obvious timezone contexts (prefer dictionary over bare `CST`)
- Locale tokens: `zh-CN`, `zh_CN`, `zh-Hans`, `chinese` in Accept-Language-like body strings
- Chinese mainland mobile / ID already in secrets pack (keep; also geo-relevant)
- IP rule already exists (keep in secrets or dual-tag)
- Optional dictionary: common province/city names — ship small, off by default if high false positives

## Config sketch (`config.toml`)

```toml
[egress]
# empty = direct
proxy_url = ""  # e.g. http://127.0.0.1:7890 or socks5://127.0.0.1:1080

[headers]
# profile: "passthrough" | "region_neutral"
profile = "region_neutral"
accept_language = "en-US,en;q=0.9"

[packs]
secrets = true
region = true
```

## Code touch points

- `crates/veil/src/config.rs` — new sections
- `crates/veil/src/proxy/forward.rs` — Client builder with proxy; apply header profile after skip_request_header
- `crates/veil-engine/src/builtin.rs` (+ rules) — region pack rules
- Admin API + UI Upstream/Rules — proxy field, pack toggles, header profile
- README / README.zh-CN / CONTEXT.md — reposition copy
- Tests: proxy client construction, Accept-Language rewrite, region rule hits

## Non-goals (MVP)

- Full TLS MITM / custom CA
- Guaranteed ban immunity
- Rewriting Chinese natural language in chat to English
