---
name: dgw
description: Start, stop, and point Claude Code at Desensitization Gateway.
---

# dgw

Slash commands: `/dgw start`, `/dgw stop`, `/dgw status`, `/dgw ui`.

This plugin starts the local gateway and sets user-level `ANTHROPIC_BASE_URL` to `http://127.0.0.1:18787`.
It does not touch OAuth login files or API keys.
If the gateway cannot start, warn and continue; traffic is not desensitized.
