# ADR 0005: Redact every parseable IP address

## Status

Accepted

## Context

The built-in IP rule could redact only private ranges (RFC1918, ULA, link-local) or every address the engine can parse. The operator chose the latter, including public IPv4 and IPv6. The model therefore cannot apply public-IP knowledge (for example treating `8.8.8.8` as Google DNS) unless that address is echoed as a placeholder and later restored.

Loopback and unspecified addresses remain on the Allowlist (`127.0.0.1`, `localhost`, `0.0.0.0`, `::1`).

## Decision

The built-in IP rule tokenizes candidate spans, parses them as IPv4 or IPv6, and replaces every successful parse whose exact original spelling is not allowlisted. The placeholder type prefix is `IP`. Strings that fail to parse are left unchanged.

## Consequences

- Internal and public endpoints in prompts do not leave the trust boundary.
- The model sees stable `{{IP_<ULID>}}` tokens per Creator, so multi-turn talk about "that host" still works after restore.
- Documentation, CDN examples, and public IPs in pasted logs are redacted; operators who need the model to know a public IP must allowlist it.
