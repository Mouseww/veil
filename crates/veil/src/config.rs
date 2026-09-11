use std::path::{Path, PathBuf};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::master_key::create_secret_file;
use crate::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Desktop,
    Server,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSource {
    #[default]
    Builtin,
    Custom,
}

/// Serde-friendly rule row stored in `config.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleConfig {
    pub id: String,
    pub type_prefix: String,
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<String>>,
    #[serde(default)]
    pub source: RuleSource,
}

fn bool_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bind: String,
    pub proxy_port: u16,
    pub management_port: u16,
    pub mode: Mode,
    pub anthropic_upstream: String,
    pub openai_completions_upstream: String,
    pub openai_responses_upstream: String,
    pub request_body_limit_mib: u64,
    pub mapping_ttl_days: u64,
    pub mapping_cap_per_creator: u64,
    pub admin_token_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_key_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_client_anthropic_base_url: Option<String>,
    #[serde(default)]
    pub rules: Vec<RuleConfig>,
    #[serde(default = "default_allowlist")]
    pub allowlist: Vec<String>,
}

fn default_allowlist() -> Vec<String> {
    vec![
        "127.0.0.1".to_string(),
        "localhost".to_string(),
        "0.0.0.0".to_string(),
        "::1".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            proxy_port: 18787,
            management_port: 18788,
            mode: Mode::Desktop,
            anthropic_upstream: "https://api.anthropic.com".to_string(),
            openai_completions_upstream: "https://api.openai.com".to_string(),
            openai_responses_upstream: "https://api.openai.com".to_string(),
            request_body_limit_mib: 32,
            mapping_ttl_days: 90,
            mapping_cap_per_creator: 100_000,
            admin_token_hash: String::new(),
            tls_cert_path: None,
            tls_key_path: None,
            saved_client_anthropic_base_url: None,
            rules: Vec::new(),
            allowlist: default_allowlist(),
        }
    }
}

impl Config {
    pub fn load(data_dir: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("config.toml");
        let mut cfg = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            toml::from_str(&text)?
        } else {
            Config::default()
        };

        let mut dirty = !path.exists();
        let desktop = env_mode().unwrap_or(cfg.mode) == Mode::Desktop;
        if desktop && cfg.admin_token_hash.is_empty() {
            let token = generate_admin_token();
            cfg.admin_token_hash = hex::encode(Sha256::digest(token.as_bytes()));
            let token_path = data_dir.join("admin.token");
            match create_secret_file(&token_path, token.as_bytes()) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Another desktop first-start won the create_new race — load theirs.
                    let existing = std::fs::read_to_string(&token_path)?;
                    cfg.admin_token_hash = hex::encode(Sha256::digest(existing.trim().as_bytes()));
                }
                Err(e) => return Err(e.into()),
            }
            dirty = true;
        }
        if dirty {
            cfg.save(data_dir)?;
        }

        apply_env_overrides(&mut cfg);
        cfg.clamp();
        Ok(cfg)
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), Error> {
        std::fs::create_dir_all(data_dir)?;
        let mut to_write = self.clone();
        to_write.clamp();
        let text = toml::to_string_pretty(&to_write)?;
        std::fs::write(data_dir.join("config.toml"), text)?;
        Ok(())
    }

    fn clamp(&mut self) {
        self.request_body_limit_mib = self.request_body_limit_mib.min(128);
    }
}

fn env_mode() -> Option<Mode> {
    let raw = crate::env::var("MODE").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "server" => Some(Mode::Server),
        "desktop" => Some(Mode::Desktop),
        _ => None,
    }
}

fn apply_env_overrides(cfg: &mut Config) {
    if let Ok(bind) = crate::env::var("BIND") {
        let bind = bind.trim();
        if !bind.is_empty() {
            cfg.bind = bind.to_string();
        }
    }
    if let Some(mode) = env_mode() {
        cfg.mode = mode;
    }
}

pub(crate) fn generate_admin_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::test_env::EnvLock;
    use sha2::{Digest, Sha256};

    #[test]
    fn config_toml_roundtrip_defaults() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.proxy_port, 18787);
        assert_eq!(back.management_port, 18788);
        assert_eq!(back.anthropic_upstream, "https://api.anthropic.com");
        assert_eq!(back.openai_completions_upstream, "https://api.openai.com");
        assert_eq!(back.openai_responses_upstream, "https://api.openai.com");
        assert_eq!(back.mapping_ttl_days, 90);
        assert_eq!(back.request_body_limit_mib, 32);
        assert_eq!(back.bind, "127.0.0.1");
        assert_eq!(back.mapping_cap_per_creator, 100_000);
    }

    #[test]
    fn config_load_save_roundtrip_and_env_overrides() {
        let mut env = EnvLock::acquire();
        env.unset("VEIL_BIND");
        env.unset("DGW_BIND");
        env.unset("VEIL_MODE");
        env.unset("VEIL_MODE");
        env.unset("DGW_MODE");
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.proxy_port, 18787);
        assert_eq!(cfg.management_port, 18788);
        assert_eq!(cfg.bind, "127.0.0.1");
        assert!(dir.path().join("config.toml").exists());

        env.set("VEIL_BIND", "10.0.0.5");
        env.set("VEIL_MODE", "server");
        let overridden = Config::load(dir.path()).unwrap();
        assert_eq!(overridden.bind, "10.0.0.5");
        assert_eq!(overridden.mode, super::Mode::Server);
    }

    #[test]
    fn desktop_first_start_writes_admin_token() {
        let mut env = EnvLock::acquire();
        env.unset("VEIL_MODE");
        env.unset("DGW_MODE");
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.admin_token_hash.len(), 64);
        let token = std::fs::read_to_string(dir.path().join("admin.token")).unwrap();
        let token = token.trim();
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        assert_eq!(hash, cfg.admin_token_hash);
        let again = Config::load(dir.path()).unwrap();
        assert_eq!(again.admin_token_hash, cfg.admin_token_hash);
    }

    #[test]
    fn request_body_limit_clamped_to_128() {
        let mut env = EnvLock::acquire();
        env.unset("VEIL_MODE");
        env.unset("DGW_MODE");
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.request_body_limit_mib = 999;
        cfg.save(dir.path()).unwrap();
        assert_eq!(
            Config::load(dir.path()).unwrap().request_body_limit_mib,
            128
        );
    }
}
