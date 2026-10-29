//! An opt-in Codex profile. The base configuration and login remain untouched.
use std::path::{Path, PathBuf};

use serde::Serialize;
use toml_edit::{value, DocumentMut, Item, Table};

use crate::{master_key::create_secret_file, Error};

const PROVIDER: &str = "veil_chatgpt";
const PROFILE_FILENAME: &str = "veil.config.toml";

pub fn config_path() -> PathBuf {
    let dir = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .unwrap_or_default(),
            )
            .join(".codex")
        });
    dir.join("config.toml")
}

#[derive(Serialize)]
pub struct Connection {
    pub configured: bool,
    pub profile_path: PathBuf,
    pub launch_command: &'static str,
    pub base_url: String,
}

fn profile_path() -> PathBuf {
    config_path().with_file_name(PROFILE_FILENAME)
}

pub fn status(port: u16) -> Connection {
    status_at(&profile_path(), port)
}

fn status_at(path: &Path, port: u16) -> Connection {
    let base_url = format!("http://127.0.0.1:{port}/codex");
    let doc = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.parse::<DocumentMut>().ok());
    let configured = doc.as_ref().is_some_and(|d| {
        d.get("model_provider").and_then(|v| v.as_str()) == Some(PROVIDER)
            && d.get("model_providers")
                .and_then(|v| v.get(PROVIDER))
                .is_some_and(|p| {
                    p.get("base_url").and_then(|v| v.as_str()) == Some(base_url.as_str())
                        && p.get("requires_openai_auth").and_then(|v| v.as_bool()) == Some(true)
                        && p.get("supports_websockets").and_then(|v| v.as_bool()) == Some(false)
                })
    });
    Connection {
        configured,
        profile_path: path.into(),
        launch_command: "codex --profile veil",
        base_url,
    }
}

pub fn connect(port: u16) -> Result<Connection, Error> {
    connect_at(&profile_path(), port)
}

fn connect_at(path: &Path, port: u16) -> Result<Connection, Error> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let mut doc = existing
        .parse::<DocumentMut>()
        .map_err(|_| Error::Setup("veil.config.toml is invalid; no files changed".into()))?;
    // An unrelated existing profile belongs to the user; never repurpose it.
    if !existing.trim().is_empty()
        && doc.get("model_provider").and_then(|v| v.as_str()) != Some(PROVIDER)
    {
        return Err(Error::Setup("veil.config.toml already exists and is not a Veil profile; rename it before creating this profile".into()));
    }
    if doc
        .get("model_providers")
        .is_some_and(|p| !p.is_table_like())
    {
        return Err(Error::Setup(
            "Veil profile model_providers must be a table; no files changed".into(),
        ));
    }
    // Updating a generated profile preserves any additional user settings in it.
    if let Some(p) = doc.get("model_providers").and_then(|v| v.get(PROVIDER)) {
        if !p.is_table_like()
            || p.get("requires_openai_auth").and_then(|v| v.as_bool()) != Some(true)
            || p.get("auth").is_some()
        {
            return Err(Error::Setup(
                "provider veil_chatgpt already exists; rename it before connecting".into(),
            ));
        }
    }
    doc["model_provider"] = value(PROVIDER);
    let providers = &mut doc["model_providers"];
    expand_table(providers);
    providers.as_table_mut().unwrap().set_implicit(true);
    let p = &mut providers[PROVIDER];
    expand_table(p);
    p.as_table_mut().unwrap().set_implicit(false);
    p["name"] = value("Veil · ChatGPT");
    p["base_url"] = value(format!("http://127.0.0.1:{port}/codex"));
    p["wire_api"] = value("responses");
    p["requires_openai_auth"] = value(true);
    p["supports_websockets"] = value(false);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Write atomically in the same directory, preserving restrictive permissions.
    let tmp = path.with_file_name(format!("veil.config.toml.tmp-{}", random_suffix()));
    create_secret_file(&tmp, doc.to_string().as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(status_at(path, port))
}

fn expand_table(item: &mut Item) {
    if let Some(inline) = item.as_inline_table() {
        *item = Item::Table(inline.clone().into_table());
    } else if item.is_none() {
        *item = Item::Table(Table::new());
    }
}

fn random_suffix() -> String {
    use rand::RngCore;
    format!("{:016x}", rand::thread_rng().next_u64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_does_not_modify_base_config_or_login() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("config.toml");
        let path = dir.path().join(PROFILE_FILENAME);
        let src = "# keep this comment\nmodel = 'existing-model'\nmodel_provider = 'relay'\n[model_providers.relay]\nbase_url = 'https://relay.example/v1'\nenv_key = 'RELAY_KEY'\n";
        std::fs::write(&base, src).unwrap();
        std::fs::write(dir.path().join("auth.json"), b"login-unchanged").unwrap();
        assert!(connect_at(&path, 18787).unwrap().configured);
        assert!(connect_at(&path, 18787).unwrap().configured);
        let out = std::fs::read_to_string(&path).unwrap();
        let doc = out.parse::<DocumentMut>().unwrap();
        assert!(
            doc.get("model").is_none(),
            "inherit the base model instead of overriding it"
        );
        assert_eq!(doc["model_provider"].as_str(), Some(PROVIDER));
        assert_eq!(std::fs::read_to_string(&base).unwrap(), src);
        assert_eq!(
            std::fs::read(dir.path().join("auth.json")).unwrap(),
            b"login-unchanged"
        );
        assert!(!status_at(&path, 18789).configured);
    }

    #[test]
    fn invalid_config_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PROFILE_FILENAME);
        std::fs::write(&path, "[broken").unwrap();
        assert!(connect_at(&path, 18787).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[broken");
    }

    #[test]
    fn unrelated_profile_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PROFILE_FILENAME);
        let original = "model = 'my-model'\n";
        std::fs::write(&path, original).unwrap();
        assert!(connect_at(&path, 18787).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn profile_update_preserves_extra_settings_and_changes_port() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PROFILE_FILENAME);
        connect_at(&path, 18787).unwrap();
        let src = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            format!("# custom comment\nmodel_reasoning_effort = 'low'\n{src}"),
        )
        .unwrap();
        connect_at(&path, 18887).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("# custom comment"));
        assert_eq!(
            out.parse::<DocumentMut>().unwrap()["model_reasoning_effort"].as_str(),
            Some("low")
        );
        assert!(status_at(&path, 18887).configured);
        assert!(!status_at(&path, 18787).configured);
    }
}
