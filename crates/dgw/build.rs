use std::path::Path;
use std::process::Command;

fn main() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    println!("cargo:rerun-if-changed={}", ui.join("src").display());
    println!("cargo:rerun-if-env-changed=DGW_SKIP_UI_BUILD");
    if std::env::var("DGW_SKIP_UI_BUILD").ok().as_deref() == Some("1") {
        return;
    }
    if !ui.join("package.json").exists() {
        return;
    }
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    if ui.join("node_modules").exists() {
        let st = Command::new(npm).args(["run", "build"]).current_dir(&ui).status();
        if st.ok().is_none_or(|s| !s.success()) {
            println!("cargo:warning=ui build failed; using existing dist if any");
        }
    }
}
