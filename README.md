# Veil

**Hide passwords, tokens, and other secrets when you use large models. Works with the tools already on your machine — one-click install.**

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

Out of the box it catches private keys, API keys, tokens, database URLs, login passwords, mainland mobile numbers, national IDs, and IPs. Email is off — too many false hits.

Flip a switch to pause a rule. Open a row to edit the regex, word list, or priority. Add your own at the bottom: project codenames, internal names become `{{CODENAME_…}}`.

Allowlist strings are never replaced (`localhost` is already there). If a body cannot be proven clean, Veil returns **502** and does not forward.

---

## Later

Console: **Check for updates**. Or `veil update`.

Double-click to start. `veil stop` to quit. The mapping table is encrypted on disk. Nothing is phoned home.


## UI

<img width="1437" height="723" alt="Dashboard" src="https://github.com/user-attachments/assets/8289fb9c-9857-4c49-b973-a2ab3f50a6aa" />

<img width="1408" height="715" alt="Rules" src="https://github.com/user-attachments/assets/6c35af52-f974-4115-bd7c-3a2642874670" />

<img width="1435" height="732" alt="Upstream" src="https://github.com/user-attachments/assets/3c2bed66-0ee2-4f7a-afea-3df661006938" />

Apache-2.0.
