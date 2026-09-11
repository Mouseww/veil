use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::Error;

pub const REPO: &str = "Mouseww/veil";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn asset_name() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "veil-windows-x64.exe"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "veil-macos-arm64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "veil-macos-x64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "veil-linux-arm64"
    } else {
        "veil-linux-x64"
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub asset: String,
    pub url: String,
}

pub fn parse_latest_json(
    body: &str,
    asset: &str,
) -> Result<(String, String, Option<String>), Error> {
    let v: serde_json::Value = serde_json::from_str(body)?;
    let tag = v
        .get("tag_name")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim_start_matches('v');
    if tag.is_empty() {
        return Err(Error::Setup("no release tag".into()));
    }
    let assets = v
        .get("assets")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let url = assets
        .iter()
        .find_map(|a| {
            if a.get("name").and_then(|n| n.as_str()) == Some(asset) {
                a.get("browser_download_url")
                    .and_then(|u| u.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        })
        .ok_or_else(|| Error::Setup(format!("release has no {asset}")))?;
    let sums = assets.iter().find_map(|a| {
        if a.get("name").and_then(|n| n.as_str()) == Some("SHA256SUMS") {
            a.get("browser_download_url")
                .and_then(|u| u.as_str())
                .map(str::to_string)
        } else {
            None
        }
    });
    Ok((tag.to_string(), url, sums))
}

pub fn version_newer(latest: &str, current: &str) -> bool {
    ver_tuple(latest) > ver_tuple(current)
}

fn ver_tuple(v: &str) -> (u64, u64, u64) {
    let mut it = v.trim().trim_start_matches('v').split('.');
    let n = |s: Option<&str>| s.and_then(|x| x.parse().ok()).unwrap_or(0);
    (n(it.next()), n(it.next()), n(it.next()))
}

pub fn verify_sha256(bytes: &[u8], sums: &str, asset: &str) -> bool {
    let hex = hex::encode(Sha256::digest(bytes));
    for line in sums.lines() {
        let line = line.trim();
        let mut parts = line.split_whitespace();
        let Some(h) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        let name = name.trim_start_matches('*');
        if name == asset {
            return h.eq_ignore_ascii_case(&hex);
        }
    }
    false
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("veil/{}", current_version()))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .expect("reqwest")
}

pub async fn check() -> Result<UpdateInfo, Error> {
    let asset = asset_name();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = client()
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Setup(e.to_string()))?
        .error_for_status()
        .map_err(|e| Error::Setup(e.to_string()))?
        .text()
        .await
        .map_err(|e| Error::Setup(e.to_string()))?;
    let (latest, asset_url, _) = parse_latest_json(&body, asset)?;
    Ok(UpdateInfo {
        current: current_version().into(),
        latest: latest.clone(),
        newer: version_newer(&latest, current_version()),
        asset: asset.into(),
        url: asset_url,
    })
}

pub async fn download_and_stage(dest: &Path) -> Result<UpdateInfo, Error> {
    let info = check().await?;
    if !info.newer {
        return Ok(info);
    }
    let cli = client();
    let bytes = cli
        .get(&info.url)
        .send()
        .await
        .map_err(|e| Error::Setup(e.to_string()))?
        .error_for_status()
        .map_err(|e| Error::Setup(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| Error::Setup(e.to_string()))?;
    let sums_url = format!(
        "https://github.com/{REPO}/releases/download/v{}/SHA256SUMS",
        info.latest
    );
    if let Ok(resp) = cli.get(&sums_url).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if !text.is_empty() && !verify_sha256(&bytes, &text, &info.asset) {
                    return Err(Error::Setup("SHA-256 mismatch".into()));
                }
            }
        }
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(dest, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(dest)?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(dest, p)?;
    }
    Ok(info)
}

pub fn current_exe() -> Result<PathBuf, Error> {
    std::env::current_exe().map_err(Error::from)
}

pub fn schedule_replace(current: &Path, staged: &Path) -> Result<(), Error> {
    #[cfg(windows)]
    {
        let bat = current.with_extension("update.bat");
        let script = format!(
            "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nmove /y \"{staged}\" \"{current}\"\r\nstart \"\" \"{current}\"\r\ndel \"%~f0\"\r\n",
            staged = staged.display(),
            current = current.display()
        );
        std::fs::write(&bat, script)?;
        std::process::Command::new("cmd")
            .args(["/C", "start", "/min", "", &bat.to_string_lossy()])
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let bak = current.with_extension("old");
        let _ = std::fs::remove_file(&bak);
        let _ = std::fs::rename(current, &bak);
        std::fs::copy(staged, current)?;
        let _ = std::fs::remove_file(staged);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_latest_json, verify_sha256, version_newer};
    use sha2::{Digest, Sha256};

    #[test]
    fn parses_github_release_json() {
        let body = r#"{"tag_name":"v0.3.0","assets":[{"name":"veil-linux-x64","browser_download_url":"https://github.com/Mouseww/veil/releases/download/v0.3.0/veil-linux-x64"},{"name":"SHA256SUMS","browser_download_url":"https://github.com/Mouseww/veil/releases/download/v0.3.0/SHA256SUMS"}]}"#;
        let (tag, url, sums) = parse_latest_json(body, "veil-linux-x64").unwrap();
        assert_eq!(tag, "0.3.0");
        assert!(url.contains("veil-linux-x64"));
        assert!(sums.unwrap().contains("SHA256SUMS"));
    }

    #[test]
    fn version_compare() {
        assert!(version_newer("0.3.0", "0.2.1"));
        assert!(!version_newer("0.2.0", "0.2.0"));
        assert!(!version_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn sha_ok_and_mismatch() {
        let bytes = b"hello";
        let hex = hex::encode(Sha256::digest(bytes));
        let sums = format!("{hex}  veil-linux-x64");
        assert!(verify_sha256(bytes, &sums, "veil-linux-x64"));
        assert!(!verify_sha256(b"nope", &sums, "veil-linux-x64"));
    }
}
