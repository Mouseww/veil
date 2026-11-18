//! Opt-in local collection. Raw identifiers stay in the local config, never in API views.
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veil_engine::rules::Rule;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub enabled: bool,
    pub app_ids: bool,
    pub hardware: bool,
    pub user_context: bool,
    pub git_email: bool,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            enabled: false,
            app_ids: true,
            hardware: true,
            user_context: false,
            git_email: false,
        }
    }
}
impl Selection {
    pub fn includes(&self, category: &str) -> bool {
        match category {
            "app_ids" => self.app_ids,
            "hardware" => self.hardware,
            "user_context" => self.user_context,
            "git_email" => self.git_email,
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedRule {
    pub id: String,
    pub type_prefix: String,
    pub category: String,
    pub priority: i32,
    pub patterns: Vec<String>,
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentityConfig {
    pub selection: Selection,
    pub rules: Vec<GeneratedRule>,
    pub last_scan: Option<u64>,
}

impl std::fmt::Debug for IdentityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityConfig")
            .field("selection", &self.selection)
            .field("rule_count", &self.rules.len())
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
pub struct RuleSummary {
    pub id: String,
    pub type_prefix: String,
    pub category: String,
    pub variants: usize,
    pub active: bool,
}
#[derive(Serialize)]
pub struct IdentityView {
    #[serde(flatten)]
    pub selection: Selection,
    pub supported_platform: &'static str,
    pub rules: Vec<RuleSummary>,
    pub last_scan: Option<u64>,
    pub collection_available: bool,
    pub running_enabled: bool,
    pub running_rule_count: usize,
    pub restart_required: bool,
}

impl IdentityConfig {
    pub fn view(&self, running: &Self) -> IdentityView {
        IdentityView {
            selection: self.selection.clone(),
            supported_platform: std::env::consts::OS,
            rules: self
                .rules
                .iter()
                .map(|r| RuleSummary {
                    id: r.id.clone(),
                    type_prefix: r.type_prefix.clone(),
                    category: r.category.clone(),
                    variants: r.patterns.len(),
                    active: self.selection.enabled && self.selection.includes(&r.category),
                })
                .collect(),
            last_scan: self.last_scan,
            collection_available: home_dir().is_some(),
            running_enabled: running.selection.enabled,
            running_rule_count: running
                .rules
                .iter()
                .filter(|rule| {
                    running.selection.enabled && running.selection.includes(&rule.category)
                })
                .count(),
            restart_required: self.selection != running.selection || self.rules != running.rules,
        }
    }

    pub fn compiled_rules(&self) -> Vec<Rule> {
        if !self.selection.enabled {
            return Vec::new();
        }
        self.rules
            .iter()
            .filter(|r| self.selection.includes(&r.category))
            .map(|r| {
                Rule::regex(
                    format!("local-identity/{}", r.id),
                    &r.type_prefix,
                    &r.patterns.join("|"),
                    r.priority,
                )
            })
            .collect()
    }

    /// Retain previously protected forms if a device is temporarily unavailable or changes.
    pub fn merge(&mut self, incoming: Vec<GeneratedRule>) -> Result<(), &'static str> {
        for new in incoming {
            if let Some(old) = self.rules.iter_mut().find(|r| r.id == new.id) {
                for pattern in new.patterns {
                    if !old.patterns.contains(&pattern) {
                        if old.patterns.len() >= 32 {
                            return Err("local identity history is full; review the local config before refreshing");
                        }
                        old.patterns.push(pattern);
                    }
                }
            } else {
                self.rules.push(new);
            }
        }
        self.rules.sort_by(|a, b| a.id.cmp(&b.id));
        self.last_scan = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct Collected {
    pub device_ids: Vec<String>,
    pub machine_ids: Vec<String>,
    pub installs: Vec<String>,
    pub machine_guid: Vec<String>,
    pub host: String,
    pub user: String,
    pub sid: String,
    pub smbios: Vec<String>,
    pub sqm: Vec<String>,
    pub product: Vec<String>,
    pub disks: Vec<String>,
    pub volumes: Vec<String>,
    pub macs: Vec<String>,
    pub serials: Vec<String>,
    pub emails: Vec<String>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

// Subprocesses receive no identifier input; output and errors never enter logs.
fn output(mut command: Command, cwd: &Path, timeout: Duration) -> Option<String> {
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        stdout
            .by_ref()
            .take(1024 * 1024)
            .read_to_end(&mut data)
            .ok()?;
        Some(data)
    });
    let start = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let data = reader.join().ok()??;
    if !success {
        return None;
    }
    String::from_utf8(data)
        .ok()
        .map(|s| s.trim_matches('\u{feff}').trim().to_string())
}

fn small_text(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim_matches('\u{feff}').trim().to_string())
}

pub fn collect(selection: &Selection) -> Collected {
    let mut values = Collected::default();
    let Some(home) = home_dir() else {
        return values;
    };
    if selection.app_ids {
        let mut config_paths = vec![home.join(".claude.json")];
        if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
            config_paths.push(PathBuf::from(dir).join(".claude.json"));
        }
        for path in config_paths {
            if let Some(json) =
                small_text(&path).and_then(|s| serde_json::from_str::<Value>(&s).ok())
            {
                if let Some(id) = json.get("userID").and_then(Value::as_str) {
                    values.device_ids.push(id.into());
                }
                if let Some(id) = json.get("machineID").and_then(Value::as_str) {
                    values.machine_ids.push(id.into());
                }
            }
        }
        let bases: Vec<PathBuf> = if cfg!(windows) {
            ["LOCALAPPDATA", "APPDATA"]
                .iter()
                .filter_map(std::env::var_os)
                .map(PathBuf::from)
                .collect()
        } else if cfg!(target_os = "macos") {
            vec![home.join("Library/Application Support")]
        } else {
            vec![std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))]
        };
        for base in bases {
            for app in ["Claude", "Claude-3p"] {
                if let Some(encoded) = small_text(&base.join(app).join("ant-did")) {
                    if let Ok(decoded) = STANDARD.decode(encoded.as_bytes()) {
                        if let Ok(uuid) = String::from_utf8(decoded) {
                            values.installs.push(uuid);
                        }
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    if selection.hardware || selection.user_context {
        let mut command = Command::new(
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into()))
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        );
        let script = format!(
            "$collectHardware=${};$collectContext=${};\n{}",
            selection.hardware,
            selection.user_context,
            include_str!("local_identity/windows.ps1")
        );
        command.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        if let Some(json) = output(command, &home, Duration::from_secs(15))
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            let strings = |key: &str| -> Vec<String> {
                match &json[key] {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(a) => a
                        .iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect(),
                    _ => Vec::new(),
                }
            };
            values.host = strings("host").into_iter().next().unwrap_or_default();
            values.user = strings("user").into_iter().next().unwrap_or_default();
            values.sid = strings("sid").into_iter().next().unwrap_or_default();
            values.machine_guid = strings("machine_guid");
            values.smbios = strings("smbios");
            values.sqm = strings("sqm");
            values.product = strings("product");
            values.disks = strings("disks");
            values.volumes = strings("volumes");
            values.macs = strings("macs");
            values.serials = strings("serials");
        }
    }
    if selection.user_context {
        if values.user.is_empty() {
            values.user =
                std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).unwrap_or_default();
        }
        if values.host.is_empty() {
            let mut command = Command::new("hostname");
            command.env("LC_ALL", "C");
            values.host = output(command, &home, Duration::from_secs(2)).unwrap_or_default();
        }
    }
    if selection.git_email {
        let mut command = Command::new("git");
        command
            .args(["config", "--global", "--get", "user.email"])
            .env_remove("GIT_CONFIG_PARAMETERS")
            .env_remove("GIT_CONFIG_COUNT");
        if let Some(email) = output(command, &home, Duration::from_secs(3)) {
            values.emails.push(email);
        }
    }
    values
}

fn valid_text(s: &str, min: usize) -> bool {
    s.len() >= min && s.len() <= 512 && !s.chars().any(char::is_control)
}
fn meaningful(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    valid_text(s, 5)
        && ![
            "o.e.m",
            "oem",
            "default",
            "to be filled",
            "system serial",
            "chassis serial",
            "base board serial",
            "not applicable",
        ]
        .iter()
        .any(|x| lower.contains(x))
        && !["none", "n/a", "unknown", "not specified"].contains(&lower.as_str())
        && s.chars()
            .any(|c| c != '0' && c != '.' && c != '-' && !c.is_whitespace())
        && !s.starts_with("1234")
}
fn uuid_forms(raw: &str) -> Vec<String> {
    let s = raw.trim().trim_matches(['{', '}']).to_ascii_lowercase();
    let h = s.replace('-', "");
    if h.len() != 32
        || !h.chars().all(|c| c.is_ascii_hexdigit())
        || h.chars().all(|c| c == '0')
        || h.chars().all(|c| c == 'f')
    {
        return Vec::new();
    }
    let canonical = format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    );
    if s != h && s != canonical {
        return Vec::new();
    }
    vec![canonical, h]
}
fn hex_id(raw: &str) -> Option<String> {
    if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(raw.to_ascii_lowercase())
    } else {
        None
    }
}
fn alternatives(values: impl IntoIterator<Item = String>) -> String {
    let set: BTreeSet<String> = values.into_iter().collect();
    let mut sorted: Vec<_> = set.into_iter().collect();
    sorted.sort_by_key(|v| std::cmp::Reverse(v.len()));
    sorted
        .into_iter()
        .map(|v| regex::escape(&v))
        .collect::<Vec<_>>()
        .join("|")
}
fn literal_pattern(values: Vec<String>, boundary: &str) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    Some(format!(
        "(?i)(?<![{boundary}])(?:{})(?![{boundary}])",
        alternatives(values)
    ))
}

pub fn generate(v: &Collected, selection: &Selection) -> Vec<GeneratedRule> {
    let mut out = Vec::new();
    let mut add =
        |id: &str, prefix: &str, category: &str, priority: i32, pattern: Option<String>| {
            if selection.includes(category) {
                if let Some(pattern) = pattern {
                    out.push(GeneratedRule {
                        id: id.into(),
                        type_prefix: prefix.into(),
                        category: category.into(),
                        priority,
                        patterns: vec![pattern],
                    });
                }
            }
        };
    add(
        "device-id",
        "DEVICEID",
        "app_ids",
        160,
        literal_pattern(
            v.device_ids.iter().filter_map(|s| hex_id(s)).collect(),
            "0-9a-f",
        ),
    );
    let mids: Vec<String> = v
        .machine_ids
        .iter()
        .filter_map(|s| hex_id(s))
        .flat_map(|s| vec![s[..12].into(), s])
        .collect();
    add(
        "machine-id",
        "MACHINEID",
        "app_ids",
        160,
        literal_pattern(mids, "0-9a-f"),
    );
    let installs: Vec<String> = v.installs.iter().flat_map(|s| uuid_forms(s)).collect();
    let install_pattern = if installs.is_empty() {
        None
    } else {
        let b64: Vec<String> = v
            .installs
            .iter()
            .filter(|s| !uuid_forms(s).is_empty())
            .map(|s| STANDARD.encode(s.as_bytes()))
            .collect();
        Some(format!(
            "(?<![A-Za-z0-9])(?:(?i:{})|{})(?![A-Za-z0-9])",
            alternatives(installs),
            alternatives(b64)
        ))
    };
    add(
        "desktop-install-id",
        "INSTALLID",
        "app_ids",
        160,
        install_pattern,
    );
    for (id, prefix, values, priority) in [
        ("windows-machine-guid", "WINID", &v.machine_guid, 162),
        ("smbios-uuid", "HWUUID", &v.smbios, 162),
        ("sqm-machine-id", "SQMID", &v.sqm, 162),
    ] {
        add(
            id,
            prefix,
            "hardware",
            priority,
            literal_pattern(
                values.iter().flat_map(|s| uuid_forms(s)).collect(),
                "A-Za-z0-9",
            ),
        );
    }
    add(
        "windows-product-id",
        "PRODUCTID",
        "hardware",
        160,
        literal_pattern(
            v.product
                .iter()
                .filter(|s| meaningful(s))
                .cloned()
                .collect(),
            "A-Za-z0-9-",
        ),
    );
    let sid = v.sid.trim();
    let parts: Vec<_> = sid.split('-').collect();
    let sid_pattern = if parts.len() == 8
        && parts[..4] == ["S", "1", "5", "21"]
        && parts[4..]
            .iter()
            .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
    {
        Some(format!(
            "(?<![0-9-]){}(?![0-9])",
            regex::escape(sid.rsplit_once('-').unwrap().0)
        ))
    } else {
        None
    };
    add("user-sid", "USERSID", "hardware", 160, sid_pattern);
    let mut disks = Vec::new();
    for raw in &v.disks {
        let s = raw.trim().trim_end_matches('.').trim();
        if meaningful(s) && s.len() >= 6 {
            disks.push(s.into());
            let compact: String = s
                .chars()
                .filter(|c| !"_.-".contains(*c) && !c.is_whitespace())
                .collect();
            if compact.len() >= 10 {
                disks.push(compact);
            }
        }
    }
    add(
        "disk-serial",
        "DISKSN",
        "hardware",
        160,
        literal_pattern(disks, "A-Za-z0-9"),
    );
    let volumes: Vec<String> = v
        .volumes
        .iter()
        .filter(|s| s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit()))
        .flat_map(|s| vec![s.clone(), format!("{}-{}", &s[..4], &s[4..])])
        .collect();
    add(
        "volume-serial",
        "VOLSN",
        "hardware",
        155,
        literal_pattern(volumes, "0-9A-Fa-f-"),
    );
    let macs: BTreeSet<String> = v
        .macs
        .iter()
        .filter_map(|s| {
            let h = s.replace(['-', ':'], "");
            if h.len() != 12
                || !h.chars().all(|c| c.is_ascii_hexdigit())
                || h.chars().all(|c| c == '0')
            {
                return None;
            }
            Some(
                (0..6)
                    .map(|i| h[i * 2..i * 2 + 2].to_string())
                    .collect::<Vec<_>>()
                    .join("[-:]?"),
            )
        })
        .collect();
    add(
        "mac-address",
        "MAC",
        "hardware",
        155,
        if macs.is_empty() {
            None
        } else {
            Some(format!(
                "(?i)(?<![0-9a-f])(?:{})(?![0-9a-f])",
                macs.into_iter().collect::<Vec<_>>().join("|")
            ))
        },
    );
    add(
        "bios-serial",
        "BIOSSN",
        "hardware",
        160,
        literal_pattern(
            v.serials
                .iter()
                .map(|s| s.trim())
                .filter(|s| meaningful(s))
                .map(String::from)
                .collect(),
            "A-Za-z0-9",
        ),
    );
    if valid_text(&v.user, 1) {
        let user = regex::escape(&v.user);
        let slash = regex::escape(&char::from(92).to_string());
        let mut paths = vec![
            "(?<=Users/)".to_string(),
            format!("(?<=Users{slash})"),
            format!("(?<=Users{slash}{slash})"),
        ];
        let mut accounts = vec!["(?<=USERNAME=)".to_string(), "(?<=USER=)".into()];
        if valid_text(&v.host, 1) {
            accounts.push(format!("(?<={}{slash})", regex::escape(&v.host)));
        }
        if !cfg!(windows) {
            paths.extend([r"(?<=/home/)".into(), r"(?<=/Users/)".into()]);
        }
        add(
            "username-path",
            "USERNAME",
            "user_context",
            165,
            Some(format!(
                r#"(?im)(?:(?:{}){user}(?=[{slash}/]|$)|(?<=Users-){user}(?=-|$)|(?:{}){user}(?=$|[\s"';,])|^{user}(?=\r?$))"#,
                paths.join("|"),
                accounts.join("|")
            )),
        );
    }
    add(
        "hostname",
        "HOSTNAME",
        "user_context",
        160,
        literal_pattern(
            if valid_text(&v.host, 1) {
                vec![v.host.clone()]
            } else {
                vec![]
            },
            "A-Za-z0-9_-",
        ),
    );
    let emails = v
        .emails
        .iter()
        .filter(|s| valid_text(s, 5) && s.contains('@') && !s.chars().any(char::is_whitespace))
        .cloned()
        .collect();
    add(
        "git-email",
        "GITEMAIL",
        "git_email",
        160,
        literal_pattern(emails, "A-Za-z0-9_.+-"),
    );
    out
}
