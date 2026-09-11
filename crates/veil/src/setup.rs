use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::config::Config;
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

/// If the client already pointed at a custom API, make that Veil's Anthropic upstream.
pub fn apply_chained_upstream(cfg: &mut Config, previous: Option<&str>) -> bool {
    let Some(url) = previous
        .map(str::trim)
        .filter(|u| !u.is_empty() && *u != DEFAULT_PROXY_URL)
    else {
        return false;
    };
    cfg.saved_client_anthropic_base_url = Some(url.to_string());
    let origin = url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string();
    cfg.anthropic_upstream = origin.clone();
    cfg.openai_completions_upstream = origin.clone();
    cfg.openai_responses_upstream = origin;
    true
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
    use super::{
        apply_chained_upstream, apply_settings_json, write_claude_base_url, DEFAULT_PROXY_URL,
    };
    use crate::config::Config;

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

    #[test]
    fn chains_custom_upstream_and_ignores_local_proxy() {
        let mut cfg = Config::default();
        assert!(apply_chained_upstream(
            &mut cfg,
            Some("https://litellm.example/v1")
        ));
        assert_eq!(cfg.anthropic_upstream, "https://litellm.example");
        assert_eq!(cfg.openai_completions_upstream, "https://litellm.example");
        assert_eq!(
            cfg.saved_client_anthropic_base_url.as_deref(),
            Some("https://litellm.example/v1")
        );
        assert!(!apply_chained_upstream(&mut cfg, Some(DEFAULT_PROXY_URL)));
        assert!(!apply_chained_upstream(&mut cfg, Some("")));
    }
}
