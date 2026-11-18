# Veil

**Lower geo/environment risk signals on Claude-style official subscriptions; also hide passwords and tokens. One-click install for everyday tools.**

This does not guarantee you will not be banned. Egress IP needs an outbound proxy; bodies and headers such as `Accept-Language` are rewritten via rule packs and header profile.

You drop `.env` into the chat so it can fix a bug. It does — and the password is already sitting on someone else's server.

Veil hangs in the doorway of this machine. Secrets go out under an alias. When the answer walks back in, the alias is swapped for the original. You still read plaintext. Outside, the model only ever saw aliases.

[Download](https://github.com/Mouseww/veil/releases/latest) · [中文说明](README.zh-CN.md) · Apache-2.0 · no telemetry

---

## In one glance

You type:

```
check mysql://root:Pass123@10.1.2.3:3306/app, call 13800138000
```

The model actually receives:

```
check {{CONNSTR_01ARZ3NDEKTSV4RRFFQ69G5FAV}}, call {{PHONE_01ARZ3NDEKTSV4RRFFQ69G5FAV}}
```

It can still reason and advise. By the time the reply hits your screen, the DSN and the number are real again.

The same secret keeps the same alias for the whole conversation. It will not split one person into two.

**Do not hand Veil a key.** Login and tokens stay in the original app. The curtain covers the body; the ID card (auth headers) walks out as-is.

---

## One line, then you are in

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

When the browser opens http://127.0.0.1:18788 , it is installed. In a new terminal:

```bat
veil setup
```

Pick the apps you use. Veil reads each app's **current** API address (official or a custom relay), opens a local port for it, and points the app there. Different tools, different upstreams — they do not overwrite each other. Restart those apps and chat as usual. Official Claude login needs no extra key.

You can also click **Point this app at Veil** on the console **Upstream** tabs. Same result.

Or grab `veil-windows-x64.exe` from [Releases](https://github.com/Mouseww/veil/releases/latest) and double-click.

---

## Not on the list? Point it yourself

Veil is not glued to a handful of agents. Any tool that can set a **Base URL** (or an env var) can walk through.

Change the tool's API address to one of these. **Keep the same key:**

| What the tool speaks | Put this |
|---|---|
| Anthropic (Claude Code and kin) | `http://127.0.0.1:18787` |
| OpenAI-compatible (Cursor, Codex, most relays) | `http://127.0.0.1:18787/v1` |

The console is for you: http://127.0.0.1:18788

Company relay / LiteLLM: set the relay root on the **Upstream** page; the key in the tool is the relay's key.

---

## You decide who gets an alias

Open the console → **Rules**.

Built-in packs: `secrets` (keys, tokens, connection strings, passwords, mainland mobile/IDs, IPs) and `region` (time zones like Asia/Shanghai, locales like zh-CN). Email is off by default. Tune `[packs]`, `[egress]`, and `[headers]` in `config.toml`; restart after proxy/header changes.

Flip a switch to pause a rule. Open a row to edit the regex, word list, or priority. Add your own at the bottom: project codenames, internal names become `{{CODENAME_…}}`.

Allowlist strings are never replaced (`localhost` is already there). If a body cannot be proven clean, Veil returns **502** and does not forward.

### Local identity protection (optional)

In **Rules → Local identity protection**, sign in with the admin token, choose the categories, and click **Save settings**. The checkbox sets what Veil should load on its next start; saving does not start or restart Veil. The page separately reports the running process and saved settings, and says when a restart is needed. **Reread local identifiers** collects current values again after hardware changes or reinstallation; unchanged values do not require another restart.

Protection is off by default. Collection reads only the computer running Veil, when saving enabled protection or refreshing, never per request. Windows can collect these items:

| What is protected | Matching forms |
| --- | --- |
| Claude local `userID` and `machineID` | Exact 64-character IDs; `machineID` also matches its 12-character prefix |
| Claude Desktop installation ID | UUID with or without hyphens, plus the stored Base64 form |
| Windows installation, SMBIOS system and SQM IDs | Exact UUIDs with or without hyphens |
| Windows product ID | Exact product ID, not a Windows activation key |
| Windows account SID | This machine's SID prefix; the account suffix is preserved |
| Disk, volume, BIOS, board and chassis serials | Exact collected values and supported normalized forms; generic OEM values are skipped |
| Physical network adapter MAC addresses | Hyphenated, colon-separated or unseparated forms |
| Username and computer name (optional) | Username in user paths, account assignments or its own line; complete computer name |
| Global Git email (optional) | Only the email configured in `git config --global user.email` |

macOS and Linux support available Claude IDs, username/computer name and Git email; hardware collection is currently Windows-only. Deselect a category if it has neither available values nor saved history. Refresh retains old identifiers when a device is temporarily unavailable, and identical scans do not add duplicate pattern groups. Built-in and handwritten rules remain separate.

Collected matching patterns are stored locally in `config.toml`, which contains identifying information: do not share it. The authenticated identity API and its UI return only descriptions, placeholder types, counts and states, never the identifiers or regex patterns. Collection is unavailable in server mode. This covers recognized text values passing through Veil; it does not guarantee coverage of images, arbitrary encodings, authentication headers or traffic bypassing Veil.

---

## Later

Console: **Check for updates**. Or `veil update`.

Double-click to start. `veil stop` to quit. The mapping table is encrypted on disk. Nothing is phoned home.


## UI

<img width="1437" height="723" alt="Dashboard" src="https://github.com/user-attachments/assets/8289fb9c-9857-4c49-b973-a2ab3f50a6aa" />

<img width="1408" height="715" alt="Rules" src="https://github.com/user-attachments/assets/6c35af52-f974-4115-bd7c-3a2642874670" />

<img width="1435" height="732" alt="Upstream" src="https://github.com/user-attachments/assets/3c2bed66-0ee2-4f7a-afea-3df661006938" />

---

Thanks to everyone on LinuxDo for their support!

---

Apache-2.0.
