# Desensitization Gateway

A local reverse proxy that desensitizes outbound prompts and restores inbound model output.
Command, plugin id, and env prefix: `dgw` / `DGW_*`.

License: Apache-2.0 (`LICENSE`, `NOTICE`). No outbound telemetry.

## What it does

- Point Claude Code / OpenAI-compatible clients at `http://127.0.0.1:18787`.
- Request JSON string values are scanned (regex + dictionary + parseable IPs).
- Hits become `{{TYPE_ULID}}` placeholders; plaintext never goes upstream.
- Responses (including SSE) restore placeholders for the same Creator.
- Fail-closed: if a scannable body cannot be proven clean, the request is not forwarded.

## Desktop vs server

| Mode | Bind | Master key |
|---|---|---|
| Desktop (default) | `127.0.0.1` | auto-generated `master.key` |
| Server `DGW_MODE=server` | internal NIC | must set `DGW_MASTER_KEY` or `master.key` |

Official Anthropic subscriptions (including Team/Enterprise) should use **desktop** so OAuth stays on the machine. Shared server mode is for company API keys / LiteLLM.

## Official subscription and custom API

The gateway is always the first hop. If Claude Code already had `ANTHROPIC_BASE_URL`, that URL is saved as the Anthropic upstream (Base URL chaining). Stop/uninstall restores it. Credentials in the original request are forwarded unchanged. Login/token-refresh hosts are not intercepted.

## Run

```bash
cargo run -p dgw -- start --foreground
```

Proxy `18787`, management UI `18788`. Set `ANTHROPIC_BASE_URL=http://127.0.0.1:18787`.

Claude Code plugin: `plugin/claude-code/`. `/dgw start`. Auto-download of release binaries is SHA-256 pinned; set `DGW_NO_DOWNLOAD=1` to disable.

## Develop

```bash
cargo test --workspace
cd ui && npm test && npm run build
node --test plugin/claude-code/scripts/dgw.test.mjs
```

Glossary: [`CONTEXT.md`](CONTEXT.md). ADRs: `docs/adr/`.
