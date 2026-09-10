use std::fmt;

use http::HeaderMap;
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};

/// Identity of the mapping creator: SHA-256 of one existing credential header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Creator(pub [u8; 32]);

/// Canonical empty-credential identity. Equals anonymous_creator().
pub static ANONYMOUS: Lazy<Creator> = Lazy::new(anonymous_creator);

impl Creator {
    pub fn from_secret(s: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(s.as_bytes());
        let out = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&out);
        Creator(id)
    }

    /// First 8 hex chars (4 bytes). Safe for logs/UI; never contains the secret.
    pub fn prefix8(&self) -> String {
        hex::encode(&self.0[..4])
    }

    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Creator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.prefix8())
    }
}

/// SHA-256 of the internal anonymous label. Used when no credential header is present.
pub fn anonymous_creator() -> Creator {
    Creator::from_secret("dgw:anonymous")
}

/// Derive Creator from existing headers. Priority: x-api-key, then
/// Authorization, then api-key. Cookies are ignored.
pub fn creator_from_headers(headers: &HeaderMap) -> Creator {
    if let Some(v) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Creator::from_secret(t);
        }
    }
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let n = normalize_bearer(v);
        if !n.is_empty() {
            return Creator::from_secret(&n);
        }
    }
    if let Some(v) = headers.get("api-key").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Creator::from_secret(t);
        }
    }
    anonymous_creator()
}

fn normalize_bearer(value: &str) -> String {
    let v = value.trim();
    let scheme = "bearer";
    if v.get(..scheme.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(scheme))
    {
        let rest = &v[scheme.len()..];
        if rest.is_empty() {
            return "Bearer".to_string();
        }
        if rest.starts_with(|c: char| c.is_ascii_whitespace()) {
            return format!("Bearer {}", rest.trim());
        }
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{HeaderMap, HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn no_credentials_is_anonymous() {
        assert_eq!(creator_from_headers(&HeaderMap::new()), anonymous_creator());
        assert_eq!(anonymous_creator(), *ANONYMOUS);
        assert_ne!(anonymous_creator(), Creator([0u8; 32]));
    }

    #[test]
    fn prefers_x_api_key_over_authorization() {
        let a = creator_from_headers(&headers(&[
            ("x-api-key", "sk-ant-aaa"),
            ("authorization", "Bearer tok"),
        ]));
        let b = creator_from_headers(&headers(&[("x-api-key", "sk-ant-aaa")]));
        assert_eq!(a, b);
        let c = creator_from_headers(&headers(&[("authorization", "Bearer tok")]));
        assert_ne!(a, c);
    }

    #[test]
    fn bearer_is_case_insensitive_and_trimmed() {
        let a = creator_from_headers(&headers(&[("authorization", "Bearer abc")]));
        let b = creator_from_headers(&headers(&[("Authorization", "  bearer abc  ")]));
        assert_eq!(a, b);
    }

    #[test]
    fn ignores_cookies() {
        let a = creator_from_headers(&headers(&[("cookie", "session=secret")]));
        assert_eq!(a, anonymous_creator());
    }

    #[test]
    fn display_is_hex_prefix_only() {
        let c = creator_from_headers(&headers(&[("x-api-key", "sk")]));
        let s = c.prefix8();
        assert_eq!(s.len(), 8);
        assert!(!s.contains("sk"));
        assert_eq!(format!("{}", c), s);
        assert!(!format!("{}", c).contains("sk"));
    }

    #[test]
    fn empty_x_api_key_falls_through_to_authorization() {
        let a = creator_from_headers(&headers(&[
            ("x-api-key", "   "),
            ("authorization", "Bearer tok"),
        ]));
        let b = creator_from_headers(&headers(&[("authorization", "Bearer tok")]));
        assert_eq!(a, b);
        assert_ne!(a, anonymous_creator());
    }
}
