//! Encrypted SQLite mapping store.
//!
//! Sensitive cells are AES-256-GCM. Equality lookup uses HMAC-SHA256 under a
//! HKDF-derived mac key, never a raw hash of plaintext (ADR 0004).

mod crypto;
mod sqlite;

pub use sqlite::SqliteStore;
