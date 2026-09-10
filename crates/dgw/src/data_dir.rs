use std::path::PathBuf;

/// Resolve the process data directory. `DGW_DATA_DIR` always wins.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DGW_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    platform_data_dir().join("dgw")
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
    fn dgw_data_dir_override_wins() {
        let mut env = EnvLock::acquire();
        let tmp = tempfile::tempdir().unwrap();
        env.set("DGW_DATA_DIR", tmp.path());
        assert_eq!(default_data_dir(), tmp.path());
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_is_local_app_data_dgw() {
        let mut env = EnvLock::acquire();
        env.unset("DGW_DATA_DIR");
        let path = default_data_dir();
        let rendered = path.to_string_lossy();
        assert!(
            rendered.ends_with("\\dgw") || path.ends_with("dgw"),
            "path {path:?} must end with \\dgw"
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
