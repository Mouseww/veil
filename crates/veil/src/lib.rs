//! Veil library surface for config, data dir, and master key.

pub mod admin;
pub mod clients;
pub mod config;
pub mod data_dir;
pub mod env;
pub mod logging;
pub mod master_key;
pub mod process;
pub mod proxy;
pub mod setup;
pub mod update;

pub use config::{Config, Mode, RuleConfig};
pub use data_dir::default_data_dir;
pub use master_key::load_or_create;

pub fn crate_name() -> &'static str {
    "veil"
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no master key: set VEIL_MASTER_KEY or provide master.key in the data directory")]
    NoMasterKey,
    #[error("invalid master key")]
    InvalidMasterKey,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("config serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Setup(String),
}

#[cfg(test)]
pub(crate) mod test_env {
    use std::ffi::{OsStr, OsString};
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub struct EnvLock {
        _guard: MutexGuard<'static, ()>,
        restores: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvLock {
        pub fn acquire() -> Self {
            let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            Self {
                _guard: guard,
                restores: Vec::new(),
            }
        }

        pub fn set(&mut self, key: &'static str, val: impl AsRef<OsStr>) {
            self.note(key);
            // SAFETY: ENV_LOCK serializes all test env mutation in this process.
            unsafe { std::env::set_var(key, val) };
        }

        pub fn unset(&mut self, key: &'static str) {
            self.note(key);
            // SAFETY: ENV_LOCK serializes all test env mutation in this process.
            unsafe { std::env::remove_var(key) };
        }

        fn note(&mut self, key: &'static str) {
            if self.restores.iter().any(|(k, _)| *k == key) {
                return;
            }
            self.restores.push((key, std::env::var_os(key)));
        }
    }

    impl Drop for EnvLock {
        fn drop(&mut self) {
            for (key, prev) in self.restores.drain(..).rev() {
                // SAFETY: ENV_LOCK is still held until after Drop completes.
                unsafe {
                    match prev {
                        Some(v) => std::env::set_var(key, v),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }
}
