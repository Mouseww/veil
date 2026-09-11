# Veil — Domain Glossary

This file is a glossary only. No implementation details.

## License（许可证）

The project is licensed under Apache License 2.0.

## Product Name（产品名）

The user-facing name is Veil. The binary, Claude Code plugin id, default data-directory folder, and environment-variable prefix are `veil` / `VEIL_*`. Legacy `dgw` / `DGW_*` is still accepted.

## Gateway（脱敏网关）

A local or internal reverse proxy that sits between an AI client and an upstream model API. Clients keep their original SDK/app code and only change the API Base URL to the gateway.

## Upstream（上游）

The real model API the gateway forwards to after desensitization. v1 upstream protocols:

- Anthropic Messages API
- OpenAI Chat Completions API
- OpenAI Responses API

## Upstream Target（上游地址）

The Base URL the gateway forwards a protocol family to. It is chosen from gateway configuration, not from extra client headers or rewritten client paths. Each protocol family has one configurable target, defaulting to the vendor's official API. Unmatched protocol families are rejected rather than blindly proxied.

## Protocol Prefix Passthrough（同协议前缀透传）

Requests under a supported protocol family's path prefix are forwarded to that family's Upstream Target. Primary APIs (Messages, Chat Completions, Responses) are framed for streaming restoration. Other text/JSON bodies on the same prefix are still desensitized before forward. Multipart and file-upload bodies are rejected in v1 so secrets in attachments cannot bypass scanning.

## Token Count Request（count_tokens）

Anthropic `POST /v1/messages/count_tokens` sends a full Messages-shaped prompt in the request body and returns a token count. It is in-scope for outbound desensitization, not a metadata-only call.

## Downstream / Client（下游 / 客户端）

The caller that points its Base URL at the gateway. v1 first-party client is Claude Code, via a Skill/Plugin. Other clients may still connect by changing Base URL only.

## 上行脱敏（Outbound Desensitization）

On the request path, the gateway matches sensitive values with rules (regex and dictionaries) and replaces them with placeholders before the request leaves the trust boundary. Original plaintext must not go to the upstream. Only string values in the body are eligible for replacement. Headers, query, path, method, status, and JSON keys are forwarded unchanged.

## 下行还原（Inbound Restoration）

On the response path, the gateway replaces placeholders in the model output with the original plaintext using the mapping table. Restoration applies only to string values in the body / SSE payload, never to headers.

## Sliding-Window Restore（滑动窗口还原）

Streaming restoration that emits bytes as soon as they cannot be part of a placeholder, while retaining a bounded tail no longer than the maximum placeholder syntax. Placeholders split across SSE chunks are restored without buffering the full response. SSE event framing (`event:`, `id:`, event boundaries) is not rewritten; only payload string text is restored.

## Placeholder（占位符）

A reversible, JSON-safe token with business semantics and an unguessable unique id, e.g. `{{PHONE_01JQC...}}`, `{{IPPRIVATE_01JQC...}}`, `{{CONNSTR_01JQC...}}`. Syntax is `{{` + registered type prefix + `_` + ULID + `}}`. The character set is `[A-Z0-9_]` plus braces so the token itself never needs JSON escaping. Documentation examples such as `{{PHONE_xxxx}}` are not placeholders. The original request is never given extra headers, query params, or path segments for this purpose.

## Codec Round-Trip（编解码往返）

Matching and replacement run on decoded Unicode text (JSON escapes already resolved). Writing back re-encodes for the current context (RFC 8259 inside JSON strings). Restored plaintext is not scanned again, so a secret that contains `{{` cannot loop.

## Creator（创建者）

The identity that produced a mapping, derived from credentials the downstream request already carries. The gateway stores only a SHA-256 hash of one normalized credential, in order: `x-api-key`, then `Authorization`, then `api-key`. Other headers (including cookies) are ignored. If none are present, Creator is the internal value `anonymous`. OAuth token refresh is a new Creator; v1 does not merge Creators. Creator is not a user-facing object and is not sent as a new request parameter.

## Restoration Authorization（还原授权）

A placeholder is restored only when the current request's Creator matches the mapping's Creator. Finding a placeholder is not enough to decrypt it.

## Request Size Limit（请求体上限）

Outbound desensitization buffers the decoded request body. Default limit is 32 MiB, configurable, with a hard cap of 128 MiB. Oversize requests return 413 and are not forwarded. Streaming responses are not buffered to this limit.

## Fail-Closed（失败关闭）

Safety behavior when desensitization or restoration cannot be completed. Requests with no scannable body, or a successful scan with zero hits, still forward. If a scan cannot prove the body is free of sensitive values, or hits were found but the mapping could not be persisted, the request is not forwarded. If the Mapping Store cannot be queried at all and a syntactically valid placeholder appears, the stream is aborted rather than delivered as success. A valid placeholder that is missing, expired, or belongs to another Creator is emitted unchanged (`restore_miss`) and does not abort the stream. A single rule timeout counts as a miss for that rule only.

## Gateway Log（网关日志）

Local rotating structured logs. They may record time, protocol, method, path template, status, upstream latency, streaming flag, hit rule types and counts, Creator hash prefix, and error class. They must never record bodies, prompts, plaintext, full placeholders, credentials, the Master Key, or mapping contents. There is no outbound telemetry or crash-reporting in v1. Debug mode is bound by the same red lines.

## Mapping Table（映射表）

The confidential store that maps a placeholder to original plaintext, rule type, and Creator. Within one Creator, identical plaintext always reuses the same placeholder. Across Creators, the same plaintext gets different placeholders. The table never leaves the gateway trust boundary.

## Encrypted Persistence（加密持久化）

The mapping table is written to local encrypted storage so restoration still works after a process restart. The wrapping key that encrypts this store is not sent upstream. Sensitive cells use AES-256-GCM. Lookup indexes are HMAC-SHA256 under a HKDF-derived mac key, not raw hashes of plaintext. Placeholder ULIDs may be stored in the clear.

## Master Key（主密钥）

The operator-managed wrapping key for Encrypted Persistence. Environment variable `DGW_MASTER_KEY` wins when set. Otherwise the gateway reads a local key file. In Desktop Mode, first launch generates a random key into that file. In Server Mode, missing both the environment variable and the key file is a hard start failure for the Proxy Port. The Management UI can rotate the key (re-encrypt the Mapping Table) but never displays the current value.

## Rule（规则）

A user-manageable matcher that detects one class of sensitive value. Rules are global (shared by all Creators). Each rule has a type prefix, enable switch, source (built-in or custom), match method (regex or dictionary), and priority. Higher priority runs first; already-replaced spans are not rematched, so placeholders never nest.

## Built-in Rule Pack（内置规则包）

The default rules shipped with the gateway. They can be disabled but not deleted. They cover secrets/tokens, connection strings, China mobile numbers, national ID numbers, and IP addresses. The IP rule parses candidates as IPv4 or IPv6 and replaces every successful parse, including public addresses. Type prefix is `IP`. Email is shipped disabled because of false positives. Allowlist still excludes `127.0.0.1`, `localhost`, `0.0.0.0`, and `::1`.

## Allowlist（白名单）

Exact strings that must never be replaced. Defaults include `127.0.0.1`, `localhost`, and `0.0.0.0`.

## Rule Dry Run（规则试跑）

A Management UI action that runs the current rule set against a sample text locally and shows hits. The sample is not sent upstream.

## Proxy Port（代理端口）

The model-facing listen port. Downstream clients point their API Base URL here. It must not expose configuration or mapping plaintext. Inbound defaults to HTTP. Desktop Mode binds localhost. Server Mode may enable optional TLS with an operator-supplied certificate when binding beyond localhost. Outbound TLS follows the Upstream Target URL. v1 does not install a custom CA or intercept HTTPS to vendor hostnames. For requests whose body must be scanned or whose response must be restored, the gateway forces upstream `Accept-Encoding: identity` (or omits the header) so the stream is plaintext. This is the only request-header mutation. The client receives uncompressed payloads.

## Management Port（管理端口）

A separate listen port that serves the Management UI and management APIs. It is not the Upstream Target and is not on the model request path.

## Admin Token（管理令牌）

A secret required to change gateway configuration through the Management Port. Desktop Mode may bind the Management Port to localhost. Server Mode may bind the Proxy Port to the internal network, but binding the Management Port beyond localhost requires an Admin Token.

## Management UI（管理界面）

The Web UI used to inspect gateway status, edit rules, set the Master Key, and set Upstream Targets. v1 screens: Status, Rules (including dry run), Upstream & Access, Keys & Danger Zone, and Live Traffic. Live Traffic shows method, path template, status, streaming flag, hit rule types, and latency — never bodies, query strings, credentials, or plaintext. The UI never displays mapping plaintext or the current Master Key value.

## Official Subscription（官方订阅）

Claude Code signed in with Anthropic consumer, Team, or Enterprise plans. Authentication is the client's existing OAuth/session material, passed through unchanged. The gateway does not ask the user for an API key and does not intercept login or token-refresh hosts.

## Custom API（自定义 API）

Any operator-configured Upstream Target that speaks a supported protocol (official vendor, company proxy, LiteLLM, or other compatible endpoint). Credentials the client already sends are forwarded unchanged.

## Base URL Chaining（Base URL 衔接）

The gateway is always the client's first hop. On first enable, if the client already had an API Base URL that is not this gateway, that original URL is saved as the Upstream Target and the client is pointed at the Proxy Port. On stop or uninstall, the client Base URL is restored to the saved original. Later starts do not overwrite a human-edited Upstream Target in gateway configuration.

## Skill / Plugin（Claude Code 插件）

The Claude Code plugin that starts or points at the gateway and sets the client API Base URL to the Proxy Port. It supports Official Subscription and Custom API. It must not replace or rewrite the client's existing authentication material, and it must not intercept login or token-refresh traffic. It persists the client Base URL in user-level Claude Code settings and in plugin-contributed env, never in a project repo. The previous Base URL is restored on stop or uninstall. Start is idempotent against the pid recorded in this instance's data directory. Stop kills only that pid. If no binary is found via `DGW_BIN`, the Data Directory cache, or `PATH`, the plugin may download the plugin-pinned release tag from GitHub into the Data Directory, verify SHA-256, and only then execute it. Auto-download can be disabled. A checksum mismatch deletes the file and does not start the gateway. v1 does not require code signing.

## Data Directory（数据目录）

The local directory that holds the Mapping Store, Master Key file, pid/listen record, gateway config, logs, and the plugin's downloaded binary cache. One running process owns one Data Directory. Defaults: Windows `%LOCALAPPDATA%\veil\` (Local, not Roaming); macOS `~/Library/Application Support/veil/`; Linux `${XDG_DATA_HOME:-~/.local/share}/veil/`. `VEIL_DATA_DIR` (then `DGW_DATA_DIR`) overrides. An existing `dgw` data directory is reused if `veil` does not yet exist. It is never the current working directory or a git project folder.

## Default Ports（默认端口）

Proxy Port 18787 and Management Port 18788 unless configured otherwise. A single process listens on both.

## Config File（配置文件）

The single configuration source is `config.toml` in the Data Directory. The Management UI reads and writes this file. Rules, allowlists, Upstream Targets, bind addresses, TLS paths, TTL, and size limits live here. The Master Key does not. A small set of environment variables overrides specific operational fields (`DGW_DATA_DIR`, `DGW_MASTER_KEY`, `DGW_BIND`, `DGW_NO_DOWNLOAD`) and wins over the file. Rule and upstream edits apply without restart; bind-address and port changes require restart.

## Desktop Mode（桌面级）

The same gateway binary bound to localhost for a single developer. This is the primary path for Official Subscription and for open-source personal use. Each desktop instance has its own Mapping Table and Master Key.

## Server Mode（服务级）

The same gateway binary bound to an internal network interface for shared use inside one trust boundary (a team, department, or jump host). It is a single node with its own Mapping Table. It is not a company-wide shared mapping cluster.

## Mapping Store（映射存储）

The persistence behind the Mapping Table. v1 uses a local SQLite file with application-level encryption. Instances do not share a Mapping Store over the network.

## Mapping Retention（映射留存）

A mapping expires after 90 days of inactivity by default (no write or restore). Each Creator also has a hard cap (100,000 rows); overflow is evicted LRU. The Management UI can change TTL and wipe a Creator prefix. It cannot display plaintext.
