# Veil

[中文说明](README.zh-CN.md) · [Download](https://github.com/Mouseww/veil/releases/latest) · Apache-2.0 · No telemetry

**Your coding agent has already seen your production password.**

You paste a connection string to debug a timeout. You paste a customer phone number to write a regex. You paste an `.env` because the stack trace is ugly. That text leaves your machine, lands in a vendor log, and you cannot unsend it.

Veil sits on `localhost` between the app and the model. Secrets are replaced with placeholders **on the way out**. The model answers in placeholders. Veil puts the real values back **on the way in**. You keep working. The model never saw the plaintext.

```
Claude / Codex / Trae / PI / Hermes / Grok
        |
        v
   Veil (this laptop)     redact --> restore
        |
        v
   Anthropic / OpenAI / LiteLLM / your gateway
```

No account. No extra API key. Official Claude login still works. Third-party gateways still work. Auth headers are forwarded unchanged.

## Install (Windows, ~30s)

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

When the console opens, in a **new** terminal:

```bat
veil setup
```

Pick the apps you actually use. Restart them. Chat as usual.

Or grab `veil-windows-x64.exe` from [Releases](https://github.com/Mouseww/veil/releases/latest) and double-click it.

macOS / Linux: download the matching binary from Releases, run `./veil`, then `./veil setup`.

Proxy `http://127.0.0.1:18787` · Console `http://127.0.0.1:18788`

## When you actually need this

**Debugging with real data.** The incident is in production. The agent needs the DSN, the Redis URL, the JWT. Let it see the *shape*, not the secret.

**Customer data in the prompt.** Phone numbers, national IDs, emails in a log dump. You want a summary, not a GDPR incident.

**Internal networks.** Tickets full of `10.x` hosts, VPN endpoints, admin panels. Public IPs are redacted too; loopback stays.

**`.env` and PEM files.** You ask "why is this deploy failing?" and the private key is three lines above the error.

**Unofficial / company model gateways.** LiteLLM, a self-hosted proxy, a reseller. You still don't want *that* server to store your customers' numbers either. Point Veil at it with `--upstream`.

**Official Claude / ChatGPT-class subscriptions.** Keep the login on the laptop. Veil is only a local hop; it does not steal OAuth.

**Several coding agents at once.** Claude Code in one window, Codex in another, Trae on a second machine account. `veil setup` writes each config. One proxy, many apps.

**Demos and teaching.** Live-code in a meetup without spraying a personal token on a projector.

**Fail-closed by design.** If Veil cannot prove a request body is clean, it does **not** forward it. Better a 502 than a leak.

## Apps `veil setup` can point at Veil

| id | app |
|---|---|
| `claude` | Claude Code |
| `codex` | Codex |
| `pi` | PI Agent |
| `codebuddy` | CodeBuddy (Workbuddy) |
| `grok` | Grok Builder |
| `hermes` | Hermes |
| `trae` | Trae |

```bat
veil setup --clients claude,codex,trae
veil setup --clients all --upstream https://your-gateway.example
```

Empty `veil setup` lists what it found on this machine. Restart the apps afterwards.

## Official API vs your own gateway

| You use | What to do |
|---|---|
| Official Anthropic / Claude login | `veil setup` only |
| Already have `ANTHROPIC_BASE_URL` | `veil setup` keeps it as upstream |
| Third-party, never set a Base URL | `veil setup --upstream https://…` |
| OpenAI-compatible client | Base URL → `http://127.0.0.1:18787` (OpenAI paths use `/v1`) |

The API key / token is always the **upstream** one, stored in the app. Veil forwards `Authorization`, `x-api-key`, `api-key`. It never asks for a second secret.

Fine-tune upstreams in the console tab **Upstream**.

## What it redacts

Built-in: phone numbers, national IDs, parseable IPs (including public; `127.0.0.1` / `localhost` allowlisted), PEM blocks, `sk-` tokens, database connection strings.

Add regex or dictionary rules in the console. Dry-run a sample before you save. Placeholders look like `{{PHONE_01ARZ3NDEKTSV4RRFFQ69G5FAV}}` and only restore for the same API key (Creator).

## Commands

| | |
|---|---|
| `veil` | start and open the console |
| `veil setup` | pick apps, write their Base URL |
| `veil status` / `veil stop` | |

`VEIL_DATA_DIR`, `VEIL_MASTER_KEY`, `VEIL_BIND`, `VEIL_MODE=server` if you need them. Legacy `DGW_*` still works.

## License

Apache-2.0. No outbound telemetry. Mapping table is encrypted on disk.

