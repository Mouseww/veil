# ADR 0001: Rust for the gateway binary

## Status

Accepted

## Context

v1 must ship one binary that runs as a localhost desktop proxy and as an internal-network server. It must reverse-proxy Anthropic Messages, OpenAI Chat Completions, and OpenAI Responses, including SSE sliding-window restoration, without sending plaintext upstream. A Claude Code plugin starts this binary; users should not have to install a language runtime.

Go was the default recommendation (fast iteration, easy cross-compile). The operator chose Rust.

## Decision

The gateway process is implemented in Rust and ships as a single static-ish binary. The Management UI is embedded in that binary. The Claude Code plugin remains a separate manifest/skill/hook package and is not written in Rust.

## Consequences

- Strong streaming, memory, and cross-compile story; no Node/Python runtime on the user's machine for the proxy itself.
- v1 takes longer than a Go or TypeScript gateway.
- UI technology is a follow-on decision (server-rendered HTML vs embedded SPA vs Rust frontend framework).
