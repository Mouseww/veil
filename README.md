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

## What it hides

Built-in: phone numbers, national IDs, parseable IPs (public included, loopback allowlisted), PEM blocks, `sk-` tokens, connection strings. Add your own regex or dictionary in the console. Dry-run a sample before you save.

If Veil cannot prove a body is clean, it does **not** forward it.

## Commands

| | |
|---|---|
| `veil` | start + open the console |
| `veil setup` | write Claude Code user settings |
| `veil status` / `veil stop` | |

Env (optional): `VEIL_DATA_DIR`, `VEIL_MASTER_KEY`, `VEIL_BIND`, `VEIL_MODE=server`. Legacy `DGW_*` still works.

## License

Apache-2.0. No outbound telemetry.

