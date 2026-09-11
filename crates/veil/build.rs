use std::path::{Path, PathBuf};
use std::process::Command;

fn dist_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist")
}

fn ensure_dist_exists() {
    let dist = dist_dir();
    let index = dist.join("index.html");
    if index.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&dist);
    let _ = std::fs::write(
        index,
        b"<!doctype html><meta charset=utf-8><title>Veil</title><p>UI not built. Run npm run build in ui/.</p>",
    );
}

fn main() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    println!("cargo:rerun-if-changed={}", ui.join("src").display());
    println!("cargo:rerun-if-env-changed=VEIL_SKIP_UI_BUILD");
    println!("cargo:rerun-if-env-changed=DGW_SKIP_UI_BUILD");
    ensure_dist_exists();
    if std::env::var("VEIL_SKIP_UI_BUILD").ok().as_deref() == Some("1")
        || std::env::var("DGW_SKIP_UI_BUILD").ok().as_deref() == Some("1")
    {
        return;
    }
    if !ui.join("package.json").exists() {
        return;
    }
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    if ui.join("node_modules").exists() {
        let st = Command::new(npm)
            .args(["run", "build"])
            .current_dir(&ui)
            .status();
        if st.ok().is_none_or(|s| !s.success()) {
            println!("cargo:warning=ui build failed; using stub/existing dist");
        }
    }
    ensure_dist_exists();
}
