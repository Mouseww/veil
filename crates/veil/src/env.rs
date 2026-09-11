use std::ffi::OsString;

/// Read `VEIL_{suffix}` first, then the legacy `DGW_{suffix}`.
pub fn var(suffix: &str) -> Result<String, std::env::VarError> {
    std::env::var(format!("VEIL_{suffix}")).or_else(|_| std::env::var(format!("DGW_{suffix}")))
}

pub fn var_os(suffix: &str) -> Option<OsString> {
    std::env::var_os(format!("VEIL_{suffix}"))
        .filter(|d| !d.is_empty())
        .or_else(|| std::env::var_os(format!("DGW_{suffix}")).filter(|d| !d.is_empty()))
}
