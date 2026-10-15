//! Detect opaque base64 / data-URL payloads that must not be scanned for secrets.
//!
//! Multimodal APIs put image bytes in JSON string fields. Running phone/IP/token
//! rules over those blobs can rewrite substrings and produce invalid base64.

/// True for `data:<mime>;base64,<payload>` (any mime). The whole string is opaque.
pub fn is_data_url_base64(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 13 || !t.as_bytes().starts_with(b"data:") {
        return false;
    }
    // Avoid allocating: scan for ";base64," case-insensitively.
    let bytes = t.as_bytes();
    let needle = b";base64,";
    bytes
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

/// True when `text` is (almost) only standard or URL-safe base64 alphabet.
pub fn looks_like_base64_blob(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 32 {
        return false;
    }
    let mut n = 0usize;
    for b in t.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'-' | b'_' | b'=' => {
                n += 1;
            }
            b' ' | b'\t' | b'\n' | b'\r' => {}
            _ => return false,
        }
    }
    n >= 32
}

fn key_suggests_binary(key: &str) -> bool {
    matches!(
        key,
        "data"
            | "image"
            | "b64_json"
            | "inline_data"
            | "file_data"
            | "content_base64"
            | "audio"
            | "video"
    )
}

/// Whether outbound desensitization should leave this string untouched.
///
/// `key` is the JSON object key that owns the string, when known.
pub fn should_skip_desensitize(key: Option<&str>, text: &str) -> bool {
    if is_data_url_base64(text) {
        return true;
    }
    if !looks_like_base64_blob(text) {
        return false;
    }
    if key.is_some_and(key_suggests_binary) {
        return true;
    }
    // Raw multimodal blobs often sit under generic keys; long pure base64 is opaque.
    text.trim().len() >= 200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_data_url() {
        assert!(is_data_url_base64(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
        ));
        assert!(is_data_url_base64("data:image/jpeg;BASE64,AAAA"));
        assert!(!is_data_url_base64("data:text/plain,hello"));
        assert!(!is_data_url_base64("https://example.com/a.png"));
    }

    #[test]
    fn skips_anthropic_data_field() {
        let tiny = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        assert!(should_skip_desensitize(Some("data"), tiny));
        assert!(!should_skip_desensitize(Some("text"), tiny)); // short + non-binary key
    }

    #[test]
    fn skips_long_raw_base64_even_without_key() {
        let blob = "A".repeat(200) + &"B".repeat(56);
        assert!(should_skip_desensitize(None, &blob));
        assert!(!should_skip_desensitize(None, "13800138000"));
    }

    #[test]
    fn does_not_skip_normal_secrets() {
        assert!(!should_skip_desensitize(
            Some("content"),
            "call 13800138000 at 10.0.0.5"
        ));
        assert!(!should_skip_desensitize(
            Some("text"),
            "api_key=sk-abcdefghijklmnopqrstuvwxyz1234"
        ));
    }
}
