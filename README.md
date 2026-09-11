# Veil

[中文](README.zh-CN.md)

**You paste a database password into Claude.** It leaves your laptop. It sits in a vendor log. It trains nothing useful and burns a lot of trust.

Veil is a tiny local proxy. Point Claude Code (or any OpenAI-compatible client) at it. Phone numbers, connection strings, keys, and IDs are replaced with placeholders **before** they go to the model. The reply comes back through Veil and the real values return. The model never saw them.

```
you  -->  Veil (localhost)  -->  Anthropic / OpenAI / LiteLLM
          redact out                 restore in
```

No account. No telemetry. Official Claude subscriptions keep working — Veil does not steal your login.

## 30 seconds

**Windows**

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

Then, in a new terminal:

```bat
veil setup
```

Restart Claude Code. Chat as usual.

Or download `veil-windows-x64.exe` from [Releases](https://github.com/Mouseww/veil/releases), double-click it, run `veil setup`.

**From source**

```bash
cargo run -p veil
veil setup
```

Proxy: `http://127.0.0.1:18787`  ·  Console: `http://127.0.0.1:18788`

## Which apps

`veil setup` asks (or use `--clients`). Detected installs are pre-selected.

| id | app |
|---|---|
| `claude` | Claude Code |
| `codex` | Codex |
| `pi` | PI Agent |
| `codebuddy` | CodeBuddy (Workbuddy) |
| `grok` | Grok Builder |
| `hermes` | Hermes (Herness) |
| `trae` | Trae |

```bat
veil setup --clients claude,codex,trae
veil setup --clients all --upstream https://your-gateway.example
```

Restart each app after setup. Keys stay in the app; Veil pass-throughs them.

## Official Claude vs a custom API

The installer only starts Veil. **Where traffic goes next** is the upstream.

**Official Anthropic subscription** (Pro / Max / Team login in Claude Code): `veil setup` is enough. Your login is untouched. Veil talks to `https://api.anthropic.com`.

**Third-party / company Anthropic-compatible API** (LiteLLM, a reverse proxy, a private `ANTHROPIC_BASE_URL`):

1. Start Veil (`veil` or the installer).
2. If Claude Code already had a custom Base URL, `veil setup` **keeps it** as Veil's upstream and points the client at localhost.
3. If it never had one, say so explicitly:

```bat
veil setup --upstream https://your-gateway.example
```

You can also paste the URL in the console tab **Upstream** (`http://127.0.0.1:18788`).

**OpenAI-compatible clients:** set the client's Base URL to `http://127.0.0.1:18787`, and set Veil's OpenAI upstreams in that same console tab.

**Auth is pass-through.** Put the *upstream* API key / token in Claude Code (or `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`). Veil forwards `Authorization`, `x-api-key`, and `api-key` unchanged. It never asks you for a second key.

## What it hides

Built-in: phone numbers, national IDs, parseable IPs (public included, loopback allowlisted), PEM blocks, `sk-` tokens, connection strings. Add your own regex or dictionary in the console. Dry-run a sample before you save.

If Veil cannot prove a body is clean, it does **not** forward it.

## Commands

| | |
|---|---|
| `veil` | start + open the console |
| `veil setup` | pick apps and point them at Veil |
| `veil status` / `veil stop` | |

Env (optional): `VEIL_DATA_DIR`, `VEIL_MASTER_KEY`, `VEIL_BIND`, `VEIL_MODE=server`. Legacy `DGW_*` still works.

## License

Apache-2.0. No outbound telemetry.

