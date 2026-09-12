# Veil — a curtain in the doorway

Talking to a model sends text off this machine. Passwords, phone numbers, database URLs go with it.

**Veil is a curtain on the way out.** Secrets leave under an alias. When the reply comes in, the alias is swapped back. You still read the real words. Outside, the model only ever saw the alias.

[中文说明](README.zh-CN.md) · [Releases](https://github.com/Mouseww/veil/releases/latest) · Apache-2.0 · no telemetry

Any app that can set a Base URL can walk through it: Claude Code, Cursor, Codex, Trae, your own script, a company relay. Not a closed list.

---

## Alias out, real name back

| | Inside (you) | Alias (placeholder) | Outside (the model) |
|---|---|---|---|
| Outbound | `mysql://root:Pass123@10.1.2.3/app`, `13800138000` | `{{CONNSTR_…}}` `{{PHONE_…}}` | can reason, never sees the original |
| Inbound | you read the real DSN and number | the same alias is the same person all chat long | it thought it was discussing the alias |

Same secret, same alias, whole conversation.

Keys stay in your app. Veil aliases the **body**, not the ID card (auth headers pass through).

```
your tool  --change Base URL-->  curtain on localhost  --then out-->  vendor / relay / LiteLLM
```

---

## Install (Windows)

1. Download `veil-windows-x64.exe` from [Releases](https://github.com/Mouseww/veil/releases/latest).
2. Double-click. Browser should open http://127.0.0.1:18788
3. If GitHub file CDN is blocked:

```powershell
gh release download v0.3.5 --repo Mouseww/veil -p veil-windows-x64.exe -D $env:LOCALAPPDATA\veil\bin
Copy-Item $env:LOCALAPPDATA\veil\bin\veil-windows-x64.exe $env:LOCALAPPDATA\veil\bin\veil.exe -Force
```

| URL | For |
|---|---|
| http://127.0.0.1:18787 | **Base URL you put in the AI tool** |
| http://127.0.0.1:18788 | Console (rules / upstream / traffic) |

---

## Use any product that has a Base URL

Veil is **not** limited to the apps in `veil setup`. If a tool can set API Base URL (or `OPENAI_BASE_URL` / `ANTHROPIC_BASE_URL`), it works.

| Protocol | Base URL in the tool |
|---|---|
| Anthropic (`/v1/messages`) | `http://127.0.0.1:18787` |
| OpenAI-compatible (`/v1/chat/completions`, `/v1/responses`) | `http://127.0.0.1:18787/v1` |

1. Start Veil.
2. Point the tool at the URL above.
3. Keep using the **upstream** API key.
4. In the console **Upstream** tab, set where Veil should forward **after** redacting (`https://api.anthropic.com`, `https://api.openai.com`, or your LiteLLM / New API / company relay).
5. Send a test like `call 13800138000`. **Traffic** should show a hit; the vendor must not see the number.

`veil setup` only auto-edits configs for apps it recognizes (Claude Code, Codex, Trae, PI, Hermes, CodeBuddy, Grok). Everything else: change Base URL by hand. Same result.

**Cursor:** Settings → Models → OpenAI Base URL = `http://127.0.0.1:18787/v1`

**Claude Code:** `veil setup --clients claude` or `$env:ANTHROPIC_BASE_URL="http://127.0.0.1:18787"`

**Python:**

```python
from openai import OpenAI
client = OpenAI(base_url="http://127.0.0.1:18787/v1", api_key="upstream-key")
```

**Company relay:** Upstream tab = relay root URL; tool Base URL stays localhost; tool key = relay key.

```bat
veil setup --upstream https://api.your-relay.com
```

---

## Configure rules

Open http://127.0.0.1:18788 → **Rules**.

A rule replaces matching text with `{{TYPE_ULID}}` before the request leaves. Same API key (Creator) always gets the same placeholder, so multi-turn chat stays consistent.

| Type | Matches |
|---|---|
| `PEM` | private key blocks |
| `APIKEY` | `sk-`, `sk-ant-`, `AKIA`, `ghp_`, `api_key=` |
| `TOKEN` | `Bearer`, JWT `eyJ`, `access_token=`, `xoxb-` |
| `CONNSTR` | `mysql://`, `postgres://`, `jdbc:`, `Password=` |
| `PASSWORD` | `password:`, `passwd=`, 口令 |
| `PHONE` / `IDCARD` | CN mobile / national id |
| `IP` | parseable IPv4/IPv6 (loopback allowlisted) |
| `EMAIL` | off by default |

**Edit:** toggle on the left; click the row to expand type / priority / regex / dictionary; Save. Custom rules can be deleted. Built-in IP has no regex (it parses addresses).

**Add:** id + type prefix + regex, e.g. type `CODENAME`, pattern `\bAcmeInternal\b` → `{{CODENAME_…}}`.

**Allowlist:** one string per line that must never be replaced (`localhost` is already there).

**Dry-run:** paste a real log, run locally, **nothing is sent to the model**. Do this before you trust a new regex.

Higher priority wins on overlap. If a body cannot be proven clean, Veil returns **502** and does not forward.

---

## Commands

| | |
|---|---|
| `veil` | start + open console |
| `veil setup` | optional auto-config for detected apps |
| `veil update` | GitHub release (use console or `gh release download` if CDN is blocked) |
| `veil stop` / `veil status` | |

---

## UI
<img width="1437" height="723" alt="image" src="https://github.com/user-attachments/assets/8289fb9c-9857-4c49-b973-a2ab3f50a6aa" />
<img width="1408" height="715" alt="image" src="https://github.com/user-attachments/assets/6c35af52-f974-4115-bd7c-3a2642874670" />
<img width="1435" height="732" alt="image" src="https://github.com/user-attachments/assets/3c2bed66-0ee2-4f7a-afea-3df661006938" />




Apache-2.0. Encrypted mapping table. No telemetry.

