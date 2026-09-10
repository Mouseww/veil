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
        .get("x-dgw-admin-token")
        .and_then(|v| v.to_str().ok())
    {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("dgw_admin=") {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}
