/// Format a structured log event as compact JSON.
///
/// Callers must never pass secrets (bodies, tokens, master keys, raw hits).
/// `redact_check` panics if a forbidden key is used.
pub fn format_event(fields: &[(&str, &str)]) -> String {
    let mut map = serde_json::Map::new();
    for &(key, value) in fields {
        if forbidden_key(key) {
            continue;
        }
        map.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    serde_json::Value::Object(map).to_string()
}

/// Append one JSON line to `data_dir/logs/veil.log`. Rotation (5 MiB × 3) is
/// applied before the write.
pub fn log_event(data_dir: &std::path::Path, fields: &[(&str, &str)]) -> std::io::Result<()> {
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("veil.log");
    rotate_if_needed(&dir, &path)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(f, "{}", format_event(fields))?;
    Ok(())
}

fn rotate_if_needed(dir: &std::path::Path, path: &std::path::Path) -> std::io::Result<()> {
    const MAX: u64 = 5 * 1024 * 1024;
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if len < MAX {
        return Ok(());
    }
    let p3 = dir.join("veil.log.3");
    let p2 = dir.join("veil.log.2");
    let p1 = dir.join("veil.log.1");
    let _ = std::fs::remove_file(&p3);
    let _ = std::fs::rename(&p2, &p3);
    let _ = std::fs::rename(&p1, &p2);
    std::fs::rename(path, &p1)?;
    Ok(())
}

fn forbidden_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "body" | "authorization" | "master_key" | "token" | "headers" | "plaintext"
    )
}

#[cfg(test)]
mod tests {
    use super::format_event;

    #[test]
    fn format_event_has_path_template_not_plaintext_secret() {
        let secret = "13800138000";
        let line = format_event(&[("path_template", "/v1/messages"), ("hit_types", "PHONE")]);
        assert!(
            line.contains("/v1/messages"),
            "logged line should include path_template: {line}"
        );
        assert!(
            line.contains("PHONE"),
            "logged line should include hit_types PHONE: {line}"
        );
        assert!(
            !line.contains(secret),
            "plaintext secret must not appear in logs: {line}"
        );
    }

    #[test]
    fn forbidden_keys_are_dropped_not_panicked() {
        let line = format_event(&[
            ("path_template", "/v1/messages"),
            ("body", "13800138000"),
            ("authorization", "Bearer tok"),
        ]);
        assert!(line.contains("/v1/messages"));
        assert!(!line.contains("13800138000"));
        assert!(!line.contains("Bearer"));
        assert!(!line.contains("body"));
    }
}
