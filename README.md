# Desensitization Gateway

A local reverse proxy that desensitizes outbound prompts and restores inbound model output. The command, plugin id, and environment-variable prefix are `dgw` / `DGW_*`.

## License

Apache License 2.0. See `LICENSE` and `NOTICE`.

## Domain language

Read [`CONTEXT.md`](CONTEXT.md) for the glossary. Architectural decisions live in `docs/adr/`.

## Develop

```bash
cargo test
```

Engine crate:

```bash
cargo test -p dgw-engine
```
