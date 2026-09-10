# ADR 0003: Apache License 2.0

## Status

Accepted

## Context

The project will be open-sourced. Most users will be individuals. Companies may also run Server Mode. The gateway and Claude Code plugin will be embedded in internal toolchains. GPL/AGPL copyleft would slow company adoption and complicate plugin ecosystems. Source-available-but-not-open licenses (SSPL, BSL) would shrink contributors and confuse "open source" claims.

## Decision

The repository is licensed under Apache License 2.0.

## Consequences

- Companies can use, modify, and distribute internally with a well-understood patent grant.
- Third parties may offer hosted forks; that is accepted. The product's privacy story is local/private deployment, not exclusive SaaS.
- All source files and GitHub distribution follow Apache-2.0 notices.
