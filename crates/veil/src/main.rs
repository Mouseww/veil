//! CLI: `veil` (no args) starts and opens the UI.

use std::sync::Arc;

use tokio::net::TcpListener;
use veil::admin::{self, AdminState, TrafficLog};
use veil::config::Config;
use veil::data_dir::default_data_dir;
use veil::master_key::load_or_create;
use veil::process::{self, StartOutcome};
use veil::proxy::{router, AppState, UpstreamConfig};
use veil::setup::{self, DEFAULT_PROXY_URL};
use veil_store::SqliteStore;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("start");
    let foreground = args.iter().any(|a| a == "--foreground");
    if let Err(e) = dispatch(cmd, foreground).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn dispatch(cmd: &str, foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "start" => cmd_start(foreground).await,
        "stop" => cmd_stop(),
        "status" => cmd_status(),
        "setup" => cmd_setup(),
        "ui" => cmd_ui(true),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        _ => {
            eprintln!("unknown command: {cmd}");
            print_help();
            std::process::exit(2);
        }
    }
}

fn print_help() {
    eprintln!("Veil -- keep secrets on your machine");
    eprintln!();
    eprintln!("  Double-click veil.exe, or run:  veil");
    eprintln!("  Then run:  veil setup");
    eprintln!("  Restart Claude Code. Done.");
    eprintln!();
    eprintln!("  veil start              start in background");
    eprintln!("  veil start --foreground");
    eprintln!("  veil setup              write Claude Code user settings");
    eprintln!("  veil ui                 open the console");
    eprintln!("  veil status");
    eprintln!("  veil stop");
}

fn ui_url(cfg: &Config) -> String {
    format!("http://{}:{}/", cfg.bind, cfg.management_port)
}

fn open_browser(url: &str) {
    let _ = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
}

fn print_ready(cfg: &Config) {
    let ui = ui_url(cfg);
    println!("Veil is running");
    println!("  console: {ui}");
    println!("  proxy:   {DEFAULT_PROXY_URL}");
    println!();
    println!("Next:  veil setup");
    println!("   or set ANTHROPIC_BASE_URL={DEFAULT_PROXY_URL}");
}

async fn cmd_start(foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = default_data_dir();
    let cfg = Config::load(&data_dir)?;
    if !foreground {
        if let Some(existing) = process::read_pid(&data_dir)? {
            if process::pid_is_alive(existing.pid) && existing.exe == process::current_exe() {
                print_ready(&cfg);
                open_browser(&ui_url(&cfg));
                return Ok(());
            }
        }
        process::spawn_daemon(&process::current_exe())?;
        std::thread::sleep(std::time::Duration::from_millis(800));
        print_ready(&cfg);
        open_browser(&ui_url(&cfg));
        return Ok(());
    }
    let outcome = process::try_start(
        &data_dir,
        cfg.proxy_port,
        cfg.management_port,
        &process::current_exe(),
    )?;
    match outcome {
        StartOutcome::AlreadyRunning { .. } => {
            print_ready(&cfg);
            Ok(())
        }
        StartOutcome::Started { .. } => {
            print_ready(&cfg);
            open_browser(&ui_url(&cfg));
            serve(&data_dir, &cfg).await
        }
    }
}

async fn serve(data_dir: &std::path::Path, cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let limit = (cfg.request_body_limit_mib.saturating_mul(1024 * 1024)) as usize;
    let key = load_or_create(data_dir, cfg.mode)?;
    let store = SqliteStore::open(data_dir.join("mappings.db"), &key)
        .map_err(|e| format!("mapping store: {e}"))?;
    let state = AppState::new(Arc::new(store), UpstreamConfig::from(cfg), limit);
    let proxy = router(state);
    let loopback = cfg.bind == "127.0.0.1" || cfg.bind == "localhost" || cfg.bind == "::1";
    let admin = AdminState {
        data_dir: data_dir.to_path_buf(),
        config: Arc::new(std::sync::Mutex::new(cfg.clone())),
        loopback,
        traffic: TrafficLog::default(),
        proxy_port: cfg.proxy_port,
        management_port: cfg.management_port,
    };
    let mgmt = admin::router(admin);
    let proxy_bind = format!("{}:{}", cfg.bind, cfg.proxy_port);
    let mgmt_bind = format!("{}:{}", cfg.bind, cfg.management_port);
    let p = TcpListener::bind(&proxy_bind).await?;
    let m = TcpListener::bind(&mgmt_bind).await?;
    tokio::select! {
        r = axum::serve(p, proxy) => r?,
        r = axum::serve(m, mgmt) => r?,
    }
    Ok(())
}

fn cmd_stop() -> Result<(), Box<dyn std::error::Error>> {
    process::stop(&default_data_dir())?;
    println!("stopped");
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    match process::read_pid(&default_data_dir())? {
        Some(p) if process::pid_is_alive(p.pid) => {
            println!(
                "running pid={} proxy={} console={}",
                p.pid, p.proxy_port, p.management_port
            );
        }
        Some(p) => println!("stale pid file pid={}", p.pid),
        None => println!("not running. start with: veil"),
    }
    Ok(())
}

fn cmd_ui(open: bool) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load(&default_data_dir())?;
    let url = ui_url(&cfg);
    println!("{url}");
    if open {
        open_browser(&url);
    }
    Ok(())
}

fn cmd_setup() -> Result<(), Box<dyn std::error::Error>> {
    let path = setup::claude_settings_path();
    let prev = setup::write_claude_base_url(&path, DEFAULT_PROXY_URL)?;
    println!("wrote Claude Code user settings:");
    println!("  {}", path.display());
    println!("  ANTHROPIC_BASE_URL={DEFAULT_PROXY_URL}");
    if let Some(p) = prev {
        println!("  previous upstream noted: {p}");
    }
    println!();
    println!("Restart Claude Code. Login stays as-is.");
    Ok(())
}
