use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::Error;

pub const DEFAULT_PROXY_URL: &str = "http://127.0.0.1:18787";

pub fn claude_settings_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".claude").join("settings.json")
}

pub fn apply_settings_json(
    existing: &str,
    proxy_url: &str,
) -> Result<(String, Option<String>), Error> {
    let mut root: Value = if existing.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(existing).unwrap_or_else(|_| Value::Object(Map::new()))
    };
    if !root.is_object() {
        root = Value::Object(Map::new());
    }
    let obj = root.as_object_mut().unwrap();
    let env = obj
        .entry("env")
        .or_insert_with(|| Value::Object(Map::new()));
    if !env.is_object() {
        *env = Value::Object(Map::new());
    }
    let env_obj = env.as_object_mut().unwrap();
    let previous = env_obj
        .get("ANTHROPIC_BASE_URL")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let save = match previous.as_deref() {
        Some(u) if !u.is_empty() && u != proxy_url => Some(u.to_string()),
        _ => None,
    };
    env_obj.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        Value::String(proxy_url.to_string()),
    );
    Ok((serde_json::to_string_pretty(&root)?, save))
}

pub fn write_claude_base_url(path: &Path, proxy_url: &str) -> Result<Option<String>, Error> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let (next, previous) = apply_settings_json(&existing, proxy_url)?;
    std::fs::write(path, next)?;
    Ok(previous)
}

#[cfg(test)]
mod tests {
    use super::{apply_settings_json, write_claude_base_url, DEFAULT_PROXY_URL};

    #[test]
    fn empty_settings_gets_proxy_url() {
        let (out, prev) = apply_settings_json("", DEFAULT_PROXY_URL).unwrap();
        assert!(prev.is_none());
        assert!(out.contains(DEFAULT_PROXY_URL));
    }

    #[test]
    fn preserves_other_env_and_saves_previous_url() {
        let existing =
            "{\"env\":{\"FOO\":\"bar\",\"ANTHROPIC_BASE_URL\":\"https://litellm.example\"}}";
        let (out, prev) = apply_settings_json(existing, DEFAULT_PROXY_URL).unwrap();
        assert_eq!(prev.as_deref(), Some("https://litellm.example"));
        assert!(out.contains("FOO"));
        assert!(out.contains(DEFAULT_PROXY_URL));
        assert!(!out.contains("litellm.example"));
    }

    #[test]
    fn already_on_proxy_does_not_save_previous() {
        let existing = "{\"env\":{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:18787\"}}";
        let (_out, prev) = apply_settings_json(existing, DEFAULT_PROXY_URL).unwrap();
        assert!(prev.is_none());
    }

    #[test]
    fn writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        write_claude_base_url(&path, DEFAULT_PROXY_URL).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(DEFAULT_PROXY_URL));
    }
}
