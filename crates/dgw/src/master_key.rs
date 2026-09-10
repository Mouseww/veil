use std::path::Path;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use rand::RngCore;

use crate::config::Mode;
use crate::Error;

/// Load the 32-byte master key. `DGW_MASTER_KEY` (hex or base64) wins over `master.key`.
///
/// Never log the returned key.
pub fn load_or_create(data_dir: &Path, mode: Mode) -> Result<[u8; 32], Error> {
    if let Ok(val) = std::env::var("DGW_MASTER_KEY") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return parse_key(trimmed);
        }
    }

    let path = data_dir.join("master.key");
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_key(text.trim()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match mode {
            Mode::Server => Err(Error::NoMasterKey),
            Mode::Desktop => {
                std::fs::create_dir_all(data_dir)?;
                let mut key = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                std::fs::write(&path, hex::encode(key))?;
                restrict_secret_file(&path)?;
                Ok(key)
            }
        },
        Err(e) => Err(e.into()),
    }
}

fn parse_key(raw: &str) -> Result<[u8; 32], Error> {
    if let Ok(bytes) = hex::decode(raw) {
        if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return Ok(key);
        }
        if raw.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::InvalidMasterKey);
        }
    }
    for engine in [STANDARD, STANDARD_NO_PAD] {
        if let Ok(bytes) = engine.decode(raw) {
            if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(key);
            }
        }
    }
    Err(Error::InvalidMasterKey)
}

fn restrict_secret_file(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::load_or_create;
    use crate::config::Mode;
    use crate::test_env::EnvLock;
    use crate::Error;

    #[test]
    fn desktop_generates_master_key_file() {
        let mut env = EnvLock::acquire();
        env.unset("DGW_MASTER_KEY");
        let dir = tempfile::tempdir().unwrap();
        let key = load_or_create(dir.path(), Mode::Desktop).unwrap();
        assert_eq!(key.len(), 32);
        let file = dir.path().join("master.key");
        let hex_text = std::fs::read_to_string(&file).unwrap();
        let decoded = hex::decode(hex_text.trim()).unwrap();
        assert_eq!(decoded, key);
        let again = load_or_create(dir.path(), Mode::Desktop).unwrap();
        assert_eq!(again, key);
    }

    #[test]
    fn server_missing_key_errors_no_master_key() {
        let mut env = EnvLock::acquire();
        env.unset("DGW_MASTER_KEY");
        let dir = tempfile::tempdir().unwrap();
        let err = load_or_create(dir.path(), Mode::Server).unwrap_err();
        assert!(matches!(err, Error::NoMasterKey));
        assert!(!dir.path().join("master.key").exists());
    }

    #[test]
    fn dgw_master_key_overrides_file() {
        let mut env = EnvLock::acquire();
        let dir = tempfile::tempdir().unwrap();
        let file_key = [0x11u8; 32];
        std::fs::write(dir.path().join("master.key"), hex::encode(file_key)).unwrap();
        let env_key = [0xABu8; 32];
        env.set("DGW_MASTER_KEY", hex::encode(env_key));
        let got = load_or_create(dir.path(), Mode::Server).unwrap();
        assert_eq!(got, env_key);
    }
}
