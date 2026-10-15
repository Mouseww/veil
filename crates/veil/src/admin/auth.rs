use axum::http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};

pub fn token_authorized(
    headers: &HeaderMap,
    expected_hash: &str,
    require: bool,
) -> Result<(), StatusCode> {
    if expected_hash.is_empty() && !require {
        return Ok(());
    }
    let Some(token) = extract_token(headers) else {
        return if require {
            Err(StatusCode::UNAUTHORIZED)
        } else {
            Ok(())
        };
    };
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    if hash == expected_hash {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers
        .get("x-veil-admin-token")
        .or_else(|| headers.get("x-dgw-admin-token"))
        .and_then(|v| v.to_str().ok())
    {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    // Third-party clients often send standard Bearer auth.
    if let Some(v) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        let v = v.trim();
        if let Some(rest) = v
            .strip_prefix("Bearer ")
            .or_else(|| v.strip_prefix("bearer "))
        {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part
            .strip_prefix("veil_admin=")
            .or_else(|| part.strip_prefix("dgw_admin="))
        {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn hash(t: &str) -> String {
        hex::encode(Sha256::digest(t.as_bytes()))
    }

    #[test]
    fn accepts_bearer_authorization() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret-token"),
        );
        assert_eq!(extract_token(&h).as_deref(), Some("secret-token"));
        assert!(token_authorized(&h, &hash("secret-token"), true).is_ok());
        assert!(token_authorized(&h, &hash("other"), true).is_err());
    }

    #[test]
    fn prefers_explicit_admin_header() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-veil-admin-token",
            HeaderValue::from_static("from-header"),
        );
        h.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer from-bearer"),
        );
        assert_eq!(extract_token(&h).as_deref(), Some("from-header"));
    }
}
