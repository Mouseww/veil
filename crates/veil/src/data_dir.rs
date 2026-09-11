use std::path::PathBuf;

/// Resolve the process data directory. `VEIL_DATA_DIR` (then `DGW_DATA_DIR`) wins.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = crate::env::var_os("DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = platform_data_dir();
    let veil = base.join("veil");
    let legacy = base.join("dgw");
    if veil.exists() || !legacy.exists() {
        veil
    } else {
        legacy
    }
}

fn platform_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").unwrap_or_default();
        PathBuf::from(home).join("Library/Application Support")
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
            return PathBuf::from(xdg);
        }
        let home = std::env::var_os("HOME").unwrap_or_default();
        PathBuf::from(home).join(".local/share")
    }
}

#[cfg(test)]
mod tests {
    use super::default_data_dir;
    use crate::test_env::EnvLock;

    #[test]
    fn veil_data_dir_override_wins() {
        let mut env = EnvLock::acquire();
        let tmp = tempfile::tempdir().unwrap();
        env.set("VEIL_DATA_DIR", tmp.path());
        env.unset("DGW_DATA_DIR");
        assert_eq!(default_data_dir(), tmp.path());
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_is_local_app_data_veil() {
        let mut env = EnvLock::acquire();
        env.unset("VEIL_DATA_DIR");
        env.unset("DGW_DATA_DIR");
        let path = default_data_dir();
        let rendered = path.to_string_lossy();
        assert!(
            rendered.ends_with("\\veil") || path.ends_with("veil") || path.ends_with("dgw"),
            "path {path:?} must end with \\veil"
        );
        let local = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA must be set");
        assert!(
            path.starts_with(&local),
            "path {path:?} must be under local app data {local}"
        );
        assert!(
            !rendered.to_ascii_lowercase().contains("\\roaming\\"),
            "path {path:?} must not use Roaming app data"
        );
    }
}
