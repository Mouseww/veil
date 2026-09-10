use std::fs::OpenOptions;
use std::io::Write;
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
            Mode::Desktop => create_desktop_master_key(data_dir, &path),
        },
        Err(e) => Err(e.into()),
    }
}

fn create_desktop_master_key(data_dir: &Path, path: &Path) -> Result<[u8; 32], Error> {
    std::fs::create_dir_all(data_dir)?;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    match create_secret_file(path, hex::encode(key).as_bytes()) {
        Ok(()) => Ok(key),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another desktop first-start won the create_new race — load theirs.
            let text = std::fs::read_to_string(path)?;
            parse_key(text.trim())
        }
        Err(e) => Err(e.into()),
    }
}

/// Create a secret file exclusively. On Unix the mode is 0600 from the start
/// (`OpenOptionsExt::mode`). On Windows, `create_new` still prevents clobber races.
pub(crate) fn create_secret_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(contents)?;
    Ok(())
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

    #[cfg(unix)]
    #[test]
    fn desktop_master_key_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let mut env = EnvLock::acquire();
        env.unset("DGW_MASTER_KEY");
        let dir = tempfile::tempdir().unwrap();
        load_or_create(dir.path(), Mode::Desktop).unwrap();
        let mode = std::fs::metadata(dir.path().join("master.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
