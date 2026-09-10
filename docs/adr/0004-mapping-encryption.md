# ADR 0004: AES-256-GCM mapping cells and HMAC lookup indexes

## Status

Accepted

## Context

The Mapping Store is a local SQLite file. The Master Key lives in a key file or `DGW_MASTER_KEY`. An attacker who copies both can decrypt; that is accepted. An attacker who copies only the database must not be able to dictionary-attack phone numbers or connection strings via plaintext SHA-256 indexes.

SQLCipher was rejected to keep cross-compilation simple and to keep encryption in application code.

## Decision

- Master Key is 256 bits.
- HKDF-SHA256 derives two keys (`enc`, `mac`) with application-specific info strings.
- Plaintext columns are AES-256-GCM with a unique 96-bit nonce per row.
- Equality lookup uses HMAC-SHA256(`mac`, creator || plaintext), never raw SHA-256(plaintext).
- Placeholder ULIDs are stored unencrypted (they already leave the box toward upstream).
- Key rotation re-encrypts all rows; the old file is kept until the new one is complete.

## Consequences

- Point lookups remain possible without scanning the whole table.
- Database-only theft does not yield a phone-number rainbow table.
- Rotation is an offline/stop-the-world rewrite of the mapping file in v1.
