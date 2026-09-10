# ADR 0002: SQLite as the v1 Mapping Store

## Status

Accepted

## Context

The gateway needs encrypted, restart-safe mappings with point lookups by placeholder id and by `(creator, plaintext_hash)`. The product will be open-sourced; most users will run Desktop Mode. The same binary must also run Server Mode for a team or company jump host. A company of 10,000 people does not imply 10,000 concurrent writers against one file.

A central shared mapping database would become a company-wide key to every captured secret. That contradicts Creator-bound restoration and the trust boundary of a local gateway.

## Decision

v1 ships a single Mapping Store implementation: local SQLite (WAL) with application-layer encryption of sensitive columns. The storage is behind a `MappingStore` interface so a future Postgres backend can be added without changing protocol code.

v1 does not ship Postgres, redb, or a networked shared mapping cluster. Scale-out is more gateway processes (per person or per department), each with its own file.

## Consequences

- Desktop users get one file next to the binary; no database install.
- Server Mode is documented as single-node. A suggested concurrency ceiling belongs in ops docs, not a second storage engine.
- Enterprise "sign off" pressure may return later; that is a v2 backend, plus a new discussion of key custody and blast radius.
