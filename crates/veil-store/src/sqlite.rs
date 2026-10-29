use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use ulid::Ulid;
use veil_engine::creator::Creator;
use veil_engine::mapping::{Lookup, MappingStore, StoreError};
use veil_engine::placeholder::Placeholder;

use crate::crypto::{decrypt, derive_keys, encrypt, lookup_hmac, DerivedKeys};

const DEFAULT_TTL: Duration = Duration::from_secs(90 * 24 * 60 * 60);
const DEFAULT_CAP: u64 = 100_000;

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;
CREATE TABLE IF NOT EXISTS mappings (
  placeholder_id TEXT PRIMARY KEY,
  type_prefix TEXT NOT NULL,
  creator_hex TEXT NOT NULL,
  plaintext_hmac BLOB NOT NULL,
  plaintext_ct BLOB NOT NULL,
  last_access_unix INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_creator_hmac ON mappings(creator_hex, plaintext_hmac);
CREATE INDEX IF NOT EXISTS idx_creator_lru ON mappings(creator_hex, last_access_unix);
";

/// Encrypted SQLite mapping store. WAL mode. TTL and per-creator cap are
/// applied by purge(), not on every read/write.
pub struct SqliteStore {
    conn: Mutex<Connection>,
    keys: DerivedKeys,
    ttl: Duration,
    cap: u64,
    generation: AtomicU64,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>, master_key: &[u8; 32]) -> Result<Self, StoreError> {
        let keys = derive_keys(master_key);
        let conn = Connection::open(path.as_ref()).map_err(db_err)?;
        conn.execute_batch(SCHEMA).map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            keys,
            ttl: DEFAULT_TTL,
            cap: DEFAULT_CAP,
            generation: AtomicU64::new(0),
        })
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_cap(mut self, cap: u64) -> Self {
        self.cap = cap;
        self
    }

    /// Delete rows whose last access is older than TTL, then evict LRU extras
    /// for any creator over the cap.
    pub fn purge(&self) -> Result<(), StoreError> {
        let now = now_unix();
        let cutoff = now.saturating_sub(self.ttl.as_secs() as i64);
        let cap = i64::try_from(self.cap).unwrap_or(i64::MAX);
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute(
            "DELETE FROM mappings WHERE last_access_unix <= ?1",
            [cutoff],
        )
        .map_err(db_err)?;

        let over_cap: Vec<(String, i64)> = {
            let mut stmt = tx
                .prepare(
                    "SELECT creator_hex, COUNT(*) FROM mappings GROUP BY creator_hex HAVING COUNT(*) > ?1",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map([cap], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)?;
            rows
        };

        for (creator_hex, count) in over_cap {
            let excess = count - cap;
            if excess <= 0 {
                continue;
            }
            tx.execute(
                "DELETE FROM mappings WHERE placeholder_id IN (
                    SELECT placeholder_id FROM mappings
                    WHERE creator_hex = ?1
                    ORDER BY last_access_unix ASC, rowid ASC
                    LIMIT ?2
                )",
                params![creator_hex, excess],
            )
            .map_err(db_err)?;
        }

        tx.commit().map_err(db_err)?;
        self.generation.fetch_add(1, Ordering::Release);
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|err| StoreError::Internal(err.to_string()))
    }
}

impl MappingStore for SqliteStore {
    fn cache_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn validate_and_touch(
        &self,
        creator: &Creator,
        placeholders: &[Placeholder],
    ) -> Result<Vec<bool>, StoreError> {
        if placeholders.is_empty() {
            return Ok(Vec::new());
        }
        let creator_hex = creator.hex();
        let now = now_unix();
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(db_err)?;
        let mut valid = Vec::with_capacity(placeholders.len());
        {
            let mut select = tx
                .prepare_cached(
                    "SELECT creator_hex, plaintext_ct FROM mappings WHERE placeholder_id = ?1",
                )
                .map_err(db_err)?;
            let mut update = tx
                .prepare_cached(
                    "UPDATE mappings SET last_access_unix = ?1 WHERE placeholder_id = ?2",
                )
                .map_err(db_err)?;
            for placeholder in placeholders {
                let id = placeholder.id.to_string();
                let row = select
                    .query_row([&id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
                    })
                    .optional()
                    .map_err(db_err)?;
                let Some((owner, ct)) = row else {
                    valid.push(false);
                    continue;
                };
                if owner != creator_hex {
                    valid.push(false);
                    continue;
                }
                let plaintext =
                    decrypt(&self.keys.enc, &ct).map_err(|_| StoreError::Unavailable)?;
                String::from_utf8(plaintext).map_err(|e| StoreError::Internal(e.to_string()))?;
                update.execute(params![now, id]).map_err(db_err)?;
                valid.push(true);
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(valid)
    }

    fn get_or_insert(
        &self,
        creator: &Creator,
        type_prefix: &str,
        plaintext: &str,
    ) -> Result<Placeholder, StoreError> {
        let hmac = lookup_hmac(&self.keys.mac, creator, plaintext.as_bytes());
        let creator_hex = creator.hex();
        let conn = self.lock()?;

        for _ in 0..8 {
            let now = now_unix();
            if let Some((placeholder, id_str, ct)) = load_by_hmac(&conn, &creator_hex, &hmac)? {
                decrypt(&self.keys.enc, &ct).map_err(|_| StoreError::Unavailable)?;
                touch(&conn, &id_str, now)?;
                return Ok(placeholder);
            }

            let placeholder = Placeholder::fresh(type_prefix);
            let id_str = placeholder.id.to_string();
            let ct = encrypt(&self.keys.enc, plaintext.as_bytes());
            match conn.execute(
                "INSERT INTO mappings (
                    placeholder_id, type_prefix, creator_hex,
                    plaintext_hmac, plaintext_ct, last_access_unix
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id_str,
                    placeholder.type_prefix.as_str(),
                    creator_hex,
                    hmac.as_slice(),
                    ct,
                    now
                ],
            ) {
                Ok(_) => return Ok(placeholder),
                Err(err) if is_constraint(&err) => continue,
                Err(err) => return Err(db_err(err)),
            }
        }

        Err(StoreError::Internal(
            "repeated unique constraint on mapping insert".into(),
        ))
    }

    fn lookup(&self, creator: &Creator, placeholder: &Placeholder) -> Result<Lookup, StoreError> {
        let id_str = placeholder.id.to_string();
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT creator_hex, plaintext_ct FROM mappings WHERE placeholder_id = ?1",
                [&id_str],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(db_err)?;

        let Some((creator_hex, ct)) = row else {
            return Ok(Lookup::Miss);
        };
        if creator_hex != creator.hex() {
            return Ok(Lookup::Miss);
        }

        let pt = decrypt(&self.keys.enc, &ct).map_err(|_| StoreError::Unavailable)?;
        let text = String::from_utf8(pt).map_err(|err| StoreError::Internal(err.to_string()))?;
        touch(&conn, &id_str, now_unix())?;
        Ok(Lookup::Hit(text))
    }
}

fn load_by_hmac(
    conn: &Connection,
    creator_hex: &str,
    hmac: &[u8; 32],
) -> Result<Option<(Placeholder, String, Vec<u8>)>, StoreError> {
    conn.query_row(
        "SELECT placeholder_id, type_prefix, plaintext_ct
         FROM mappings WHERE creator_hex = ?1 AND plaintext_hmac = ?2",
        params![creator_hex, hmac.as_slice()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        },
    )
    .optional()
    .map_err(db_err)?
    .map(|(id_str, type_prefix, ct)| {
        let id = Ulid::from_string(&id_str).map_err(|err| StoreError::Internal(err.to_string()))?;
        Ok((Placeholder::new(type_prefix, id), id_str, ct))
    })
    .transpose()
}

fn touch(conn: &Connection, placeholder_id: &str, now: i64) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE mappings SET last_access_unix = ?1 WHERE placeholder_id = ?2",
        params![now, placeholder_id],
    )
    .map_err(db_err)?;
    Ok(())
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn is_constraint(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(ErrorCode::ConstraintViolation)
    )
}

fn db_err(err: rusqlite::Error) -> StoreError {
    StoreError::Internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use veil_engine::creator::{anonymous_creator, Creator};
    use veil_engine::mapping::Lookup;

    #[test]
    fn sqlite_get_or_insert_stable_and_creator_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("m.db"), &[9u8; 32]).unwrap();
        let c1 = Creator::from_secret("k1");
        let c2 = Creator::from_secret("k2");
        let p1 = store.get_or_insert(&c1, "PHONE", "13800138000").unwrap();
        let p1b = store.get_or_insert(&c1, "PHONE", "13800138000").unwrap();
        assert_eq!(p1, p1b);
        let p2 = store.get_or_insert(&c2, "PHONE", "13800138000").unwrap();
        assert_ne!(p1, p2);
        assert!(matches!(store.lookup(&c2, &p1).unwrap(), Lookup::Miss));
        assert!(matches!(store.lookup(&c1, &p1).unwrap(), Lookup::Hit(s) if s == "13800138000"));
    }

    #[test]
    fn ttl_zero_purge_makes_lookup_miss() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("m.db"), &[9u8; 32])
            .unwrap()
            .with_ttl(Duration::ZERO);
        let c = anonymous_creator();
        let p = store.get_or_insert(&c, "PHONE", "1").unwrap();
        store.purge().unwrap();
        assert!(matches!(store.lookup(&c, &p).unwrap(), Lookup::Miss));
    }

    #[test]
    fn cap_lru_evicts_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("m.db"), &[9u8; 32])
            .unwrap()
            .with_cap(2);
        let c = anonymous_creator();
        let p1 = store.get_or_insert(&c, "PHONE", "1").unwrap();
        let p2 = store.get_or_insert(&c, "PHONE", "2").unwrap();
        let p3 = store.get_or_insert(&c, "PHONE", "3").unwrap();
        store.purge().unwrap();
        assert!(matches!(store.lookup(&c, &p1).unwrap(), Lookup::Miss));
        assert!(matches!(store.lookup(&c, &p2).unwrap(), Lookup::Hit(s) if s == "2"));
        assert!(matches!(store.lookup(&c, &p3).unwrap(), Lookup::Hit(s) if s == "3"));
    }

    #[test]
    fn batch_validation_checks_owner_and_missing_ids_and_refreshes_access() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("m.db"), &[9; 32]).unwrap();
        let creator = anonymous_creator();
        let own = store
            .get_or_insert(&creator, "PHONE", "13800138000")
            .unwrap();
        let foreign = store
            .get_or_insert(&Creator::from_secret("other"), "PHONE", "13900139000")
            .unwrap();
        store
            .lock()
            .unwrap()
            .execute("UPDATE mappings SET last_access_unix = 0", [])
            .unwrap();
        let missing = Placeholder::fresh("PHONE");
        assert_eq!(
            store
                .validate_and_touch(&creator, &[own.clone(), foreign.clone(), missing])
                .unwrap(),
            [true, false, false]
        );
        let conn = store.lock().unwrap();
        let time = |p: &Placeholder| {
            conn.query_row(
                "SELECT last_access_unix FROM mappings WHERE placeholder_id = ?1",
                [p.id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        assert!(time(&own) > 0);
        assert_eq!(time(&foreign), 0);
    }

    #[test]
    fn batch_validation_authenticates_ciphertext_and_rolls_back_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("m.db"), &[9; 32]).unwrap();
        let creator = anonymous_creator();
        let good = store
            .get_or_insert(&creator, "PHONE", "13800138000")
            .unwrap();
        let bad = store
            .get_or_insert(&creator, "PHONE", "13900139000")
            .unwrap();
        {
            let conn = store.lock().unwrap();
            conn.execute("UPDATE mappings SET last_access_unix = 0", [])
                .unwrap();
            conn.execute(
                "UPDATE mappings SET plaintext_ct = ?1 WHERE placeholder_id = ?2",
                params![vec![0u8; 32], bad.id.to_string()],
            )
            .unwrap();
        }
        assert!(matches!(
            store.validate_and_touch(&creator, &[good.clone(), bad]),
            Err(StoreError::Unavailable)
        ));
        let time: i64 = store
            .lock()
            .unwrap()
            .query_row(
                "SELECT last_access_unix FROM mappings WHERE placeholder_id = ?1",
                [good.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(time, 0);
        let generation = store.cache_generation();
        store.purge().unwrap();
        assert!(store.cache_generation() > generation);
    }
}
