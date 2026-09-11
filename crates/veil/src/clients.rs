use std::path::{Path, PathBuf};

use serde_json::{Map, Value as Json};
use toml::Value as Toml;

use crate::setup::{self};
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct ClientSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    /// anthropic | openai | auto
    pub kind: &'static str,
}

pub const CLIENTS: &[ClientSpec] = &[
    ClientSpec {
        id: "claude",
        label: "Claude Code",
        aliases: &["claude-code"],
        kind: "anthropic",
    },
    ClientSpec {
        id: "codex",
        label: "Codex",
        aliases: &["openai-codex"],
        kind: "openai",
    },
    ClientSpec {
        id: "pi",
        label: "PI Agent",
        aliases: &["pi-agent"],
        kind: "auto",
    },
    ClientSpec {
        id: "codebuddy",
        label: "CodeBuddy",
        aliases: &["workbuddy", "code-buddy"],
        kind: "auto",
    },
    ClientSpec {
        id: "grok",
        label: "Grok Builder",
        aliases: &["grok-builder", "xai"],
        kind: "openai",
    },
    ClientSpec {
        id: "hermes",
        label: "Hermes",
        aliases: &["herness"],
        kind: "auto",
    },
    ClientSpec {
        id: "trae",
        label: "Trae",
        aliases: &["trae-cn"],
        kind: "auto",
    },
];

#[derive(Debug)]
pub struct ApplyResult {
    pub id: String,
    pub path: PathBuf,
    pub previous: Option<String>,
    pub note: String,
}

pub fn resolve_ids(raw: &str) -> Result<Vec<&'static str>, Error> {
    if raw.trim().eq_ignore_ascii_case("all") {
        return Ok(CLIENTS.iter().map(|c| c.id).collect());
    }
    let mut out = Vec::new();
    for part in raw.split(|c: char| c == ',' || c.is_whitespace()) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let spec = CLIENTS.iter().find(|c| {
            c.id.eq_ignore_ascii_case(part)
                || c.aliases.iter().any(|a| a.eq_ignore_ascii_case(part))
        });
        let Some(spec) = spec else {
            return Err(Error::Setup(format!("unknown client: {part}")));
        };
        if !out.contains(&spec.id) {
            out.push(spec.id);
        }
    }
    Ok(out)
}

pub fn spec(id: &str) -> Option<&'static ClientSpec> {
    CLIENTS.iter().find(|c| c.id == id)
}

/// Read the tool's current Base URL without writing.
pub fn peek_previous(id: &str) -> Option<String> {
    let raw = match id {
        "claude" => peek_json_env(&crate::setup::claude_settings_path(), "ANTHROPIC_BASE_URL"),
        "codex" => {
            let text = std::fs::read_to_string(home().join(".codex").join("config.toml")).ok()?;
            toml_table_key(&text, "model_providers.custom", "base_url")
        }
        "pi" => peek_json_field(&home().join(".pi").join("settings.json"), "baseUrl"),
        "grok" => peek_json_field(&home().join(".grok").join("config.json"), "baseUrl"),
        "hermes" => peek_json_field(&home().join(".hermes").join("config.json"), "baseUrl"),
        "codebuddy" => peek_codebuddy(),
        "trae" => peek_trae(),
        _ => None,
    }?;
    let raw = raw.trim().to_string();
    if raw.is_empty() || setup::is_local_proxy(&raw) {
        None
    } else {
        Some(raw)
    }
}

fn peek_json_env(path: &Path, key: &str) -> Option<String> {
    let v: Json = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.get("env")?.get(key)?.as_str().map(str::to_string)
}

fn peek_json_field(path: &Path, key: &str) -> Option<String> {
    let v: Json = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.get(key)?.as_str().map(str::to_string)
}

fn peek_vscode(product: &str) -> Option<String> {
    let path = vscode_user_dir(product)?.join("settings.json");
    let v: Json = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    ["openai.baseUrl", "openai.baseURL", "anthropic.baseUrl"]
        .into_iter()
        .find_map(|k| v.get(k).and_then(|x| x.as_str()).map(str::to_string))
}

fn peek_trae() -> Option<String> {
    peek_vscode("Trae CN").or_else(|| peek_vscode("Trae"))
}

fn peek_codebuddy() -> Option<String> {
    peek_vscode("CodeBuddy")
        .or_else(|| peek_json_field(&home().join(".codebuddy").join("settings.json"), "baseUrl"))
}

pub fn detected_ids() -> Vec<&'static str> {
    CLIENTS
        .iter()
        .filter(|c| is_detected(c.id))
        .map(|c| c.id)
        .collect()
}

pub fn is_detected(id: &str) -> bool {
    match id {
        "claude" => home().join(".claude").exists(),
        "codex" => home().join(".codex").exists(),
        "pi" => home().join(".pi").exists(),
        "codebuddy" => home().join(".codebuddy").exists() || vscode_user_dir("CodeBuddy").is_some(),
        "grok" => home().join(".grok").exists(),
        "hermes" => home().join(".hermes").exists(),
        "trae" => {
            home().join(".trae").exists()
                || home().join(".trae-cn").exists()
                || vscode_user_dir("Trae CN").is_some()
                || vscode_user_dir("Trae").is_some()
        }
        _ => false,
    }
}

fn home() -> PathBuf {
    PathBuf::from(
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap_or_default(),
    )
}

fn appdata() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("AppData/Roaming"))
}

fn vscode_user_dir(product: &str) -> Option<PathBuf> {
    let p = appdata().join(product).join("User");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn apply_client(id: &str, proxy: &str) -> Result<ApplyResult, Error> {
    match id {
        "claude" => apply_claude(proxy),
        "codex" => apply_codex(proxy),
        "pi" => apply_json_file(&home().join(".pi").join("settings.json"), proxy, "pi"),
        "codebuddy" => apply_codebuddy(proxy),
        "grok" => apply_json_file(&home().join(".grok").join("config.json"), proxy, "grok"),
        "hermes" => apply_json_file(&home().join(".hermes").join("config.json"), proxy, "hermes"),
        "trae" => apply_trae(proxy),
        other => Err(Error::Setup(format!("unknown client: {other}"))),
    }
}

fn openai_base(proxy: &str) -> String {
    format!("{}/v1", proxy.trim_end_matches('/'))
}

fn apply_claude(proxy: &str) -> Result<ApplyResult, Error> {
    let path = crate::setup::claude_settings_path();
    let previous = crate::setup::write_claude_base_url(&path, proxy)?;
    Ok(ApplyResult {
        id: "claude".into(),
        path,
        previous,
        note: "ANTHROPIC_BASE_URL".into(),
    })
}

fn merge_json_proxy(existing: &str, proxy: &str) -> Result<(String, Option<String>), Error> {
    let mut root: Json = if existing.trim().is_empty() {
        Json::Object(Map::new())
    } else {
        serde_json::from_str(existing).unwrap_or_else(|_| Json::Object(Map::new()))
    };
    if !root.is_object() {
        root = Json::Object(Map::new());
    }
    let obj = root.as_object_mut().unwrap();
    let prev = obj
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let openai = openai_base(proxy);
    obj.insert("baseUrl".into(), Json::String(openai.clone()));
    obj.insert("anthropicBaseUrl".into(), Json::String(proxy.into()));
    obj.insert("openaiBaseUrl".into(), Json::String(openai));
    let save = prev.filter(|u| !u.is_empty() && !setup::is_local_proxy(u));
    Ok((serde_json::to_string_pretty(&root)?, save))
}

fn apply_json_file(path: &Path, proxy: &str, id: &str) -> Result<ApplyResult, Error> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let (next, previous) = merge_json_proxy(&existing, proxy)?;
    std::fs::write(path, next)?;
    Ok(ApplyResult {
        id: id.into(),
        path: path.to_path_buf(),
        previous,
        note: "baseUrl / anthropicBaseUrl".into(),
    })
}

fn merge_vscode_settings(existing: &str, proxy: &str) -> Result<(String, Option<String>), Error> {
    let mut root: Json = if existing.trim().is_empty() {
        Json::Object(Map::new())
    } else {
        serde_json::from_str(existing).unwrap_or_else(|_| Json::Object(Map::new()))
    };
    if !root.is_object() {
        root = Json::Object(Map::new());
    }
    let obj = root.as_object_mut().unwrap();
    let prev = ["openai.baseUrl", "openai.baseURL", "anthropic.baseUrl"]
        .into_iter()
        .find_map(|k| obj.get(k).and_then(|v| v.as_str()).map(str::to_string));
    let openai = openai_base(proxy);
    obj.insert("openai.baseUrl".into(), Json::String(openai.clone()));
    obj.insert("anthropic.baseUrl".into(), Json::String(proxy.into()));
    let save = prev.filter(|u| !u.is_empty() && !setup::is_local_proxy(u));
    Ok((serde_json::to_string_pretty(&root)?, save))
}

fn apply_vscode_product(product: &str, proxy: &str, id: &str) -> Result<ApplyResult, Error> {
    let dir = vscode_user_dir(product).unwrap_or_else(|| appdata().join(product).join("User"));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("settings.json");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let (next, previous) = merge_vscode_settings(&existing, proxy)?;
    std::fs::write(&path, next)?;
    Ok(ApplyResult {
        id: id.into(),
        path,
        previous,
        note: "openai.baseUrl".into(),
    })
}

fn apply_trae(proxy: &str) -> Result<ApplyResult, Error> {
    let product = if vscode_user_dir("Trae CN").is_some() {
        "Trae CN"
    } else {
        "Trae"
    };
    apply_vscode_product(product, proxy, "trae")
}

fn apply_codebuddy(proxy: &str) -> Result<ApplyResult, Error> {
    if vscode_user_dir("CodeBuddy").is_some() {
        return apply_vscode_product("CodeBuddy", proxy, "codebuddy");
    }
    apply_json_file(
        &home().join(".codebuddy").join("settings.json"),
        proxy,
        "codebuddy",
    )
}

fn apply_codex(proxy: &str) -> Result<ApplyResult, Error> {
    let path = home().join(".codex").join("config.toml");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let openai = openai_base(proxy);
    let prev = toml_table_key(&existing, "model_providers.custom", "base_url");
    let mut next = existing;
    next = set_toml_table_key(&next, "model_providers.custom", "name", "custom");
    next = set_toml_table_key(&next, "model_providers.custom", "wire_api", "responses");
    next = set_toml_table_key(&next, "model_providers.custom", "base_url", &openai);
    next = set_toml_table_key(
        &next,
        "shell_environment_policy.set",
        "ANTHROPIC_BASE_URL",
        proxy,
    );
    next = set_toml_table_key(
        &next,
        "shell_environment_policy.set",
        "OPENAI_BASE_URL",
        &openai,
    );
    std::fs::write(&path, next)?;
    let save = prev.filter(|u| !u.is_empty() && !setup::is_local_proxy(u));
    Ok(ApplyResult {
        id: "codex".into(),
        path,
        previous: save,
        note: "model_providers.custom.base_url".into(),
    })
}

fn toml_table_key(src: &str, header: &str, key: &str) -> Option<String> {
    let head = format!("[{header}]");
    let start = src.find(&head)?;
    let rest = &src[start + head.len()..];
    let end = rest
        .find("\n[")
        .map(|i| start + head.len() + i)
        .unwrap_or(src.len());
    let section = &src[start..end];
    for line in section.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix('=') {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn set_toml_table_key(src: &str, header: &str, key: &str, value: &str) -> String {
    let head = format!("[{header}]");
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    let new_line = format!("{key} = \"{escaped}\"");
    if let Some(start) = src.find(&head) {
        let after = start + head.len();
        let end = src[after..]
            .find("\n[")
            .map(|i| after + i)
            .unwrap_or(src.len());
        let section = &src[start..end];
        let mut replaced = false;
        let mut out_section = String::new();
        for line in section.lines() {
            let t = line.trim_start();
            if !replaced && t.starts_with(key) && t[key.len()..].trim_start().starts_with('=') {
                out_section.push_str(&new_line);
                out_section.push('\n');
                replaced = true;
            } else {
                out_section.push_str(line);
                out_section.push('\n');
            }
        }
        if !replaced {
            if !out_section.ends_with('\n') {
                out_section.push('\n');
            }
            out_section.push_str(&new_line);
            out_section.push('\n');
        }
        let mut out = String::new();
        out.push_str(&src[..start]);
        out.push_str(&out_section);
        if end < src.len() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&src[end..]);
        out
    } else {
        let mut out = src.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&head);
        out.push('\n');
        out.push_str(&new_line);
        out.push('\n');
        out
    }
}

fn upsert_toml(root: &mut Toml, path: &[&str], val: Toml) {
    if path.is_empty() {
        return;
    }
    if path.len() == 1 {
        if let Toml::Table(t) = root {
            t.insert(path[0].into(), val);
        }
        return;
    }
    if !matches!(root, Toml::Table(_)) {
        *root = Toml::Table(Default::default());
    }
    let Toml::Table(t) = root else {
        return;
    };
    let entry = t
        .entry(path[0].to_string())
        .or_insert_with(|| Toml::Table(Default::default()));
    if !matches!(entry, Toml::Table(_)) {
        *entry = Toml::Table(Default::default());
    }
    upsert_toml(entry, &path[1..], val);
}

#[cfg(test)]
mod tests {
    use super::{merge_json_proxy, merge_vscode_settings, resolve_ids, upsert_toml};
    use toml::Value as Toml;

    #[test]
    fn resolve_aliases() {
        assert_eq!(
            resolve_ids("claude,workbuddy,herness").unwrap(),
            vec!["claude", "codebuddy", "hermes"]
        );
        assert!(resolve_ids("nope").is_err());
    }

    #[test]
    fn json_proxy_keeps_other_keys() {
        let (out, prev) = merge_json_proxy(
            "{\"foo\":1,\"baseUrl\":\"https://x\"}",
            "http://127.0.0.1:18787",
        )
        .unwrap();
        assert_eq!(prev.as_deref(), Some("https://x"));
        assert!(out.contains("foo"));
        assert!(out.contains("127.0.0.1:18787"));
    }

    #[test]
    fn vscode_settings_set_openai_key() {
        let (out, _) = merge_vscode_settings("{}", "http://127.0.0.1:18787").unwrap();
        assert!(out.contains("openai.baseUrl"));
        assert!(out.contains("/v1"));
    }

    #[test]
    fn toml_upsert_nested() {
        let mut root = Toml::Table(Default::default());
        upsert_toml(&mut root, &["a", "b"], Toml::String("c".into()));
        assert_eq!(root.get("a").unwrap().get("b").unwrap().as_str(), Some("c"));
    }
}
