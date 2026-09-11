//! CLI: `veil` (no args) starts and opens the UI.

use std::sync::Arc;

use std::io::{self, IsTerminal, Write};
use tokio::net::TcpListener;
use veil::admin::{self, AdminState, TrafficLog};
use veil::clients;
use veil::config::Config;
use veil::data_dir::default_data_dir;
use veil::master_key::load_or_create;
use veil::process::{self, StartOutcome};
use veil::proxy::{router, AppState, UpstreamConfig};
use veil::setup::{self, DEFAULT_PROXY_URL};
use veil::update;
use veil_store::SqliteStore;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("start");
    let foreground = args.iter().any(|a| a == "--foreground");
    if let Err(e) = dispatch(cmd, &args, foreground).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn dispatch(
    cmd: &str,
    args: &[String],
    foreground: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "start" => cmd_start(foreground).await,
        "stop" => cmd_stop(),
        "status" => cmd_status(),
        "setup" => cmd_setup(args),
        "update" => cmd_update(args).await,
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
    eprintln!();
    eprintln!("  veil start              start in background");
    eprintln!("  veil start --foreground");
    eprintln!("  veil setup [--clients LIST] [--upstream URL]");
    eprintln!("      clients: claude,codex,pi,codebuddy,grok,hermes,trae,all");
    eprintln!("  veil update [--check]   download latest GitHub release");
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
    let traffic = TrafficLog::default();
    let state = AppState::new(Arc::new(store), UpstreamConfig::from(cfg), limit)
        .with_traffic(traffic.clone());
    let proxy = router(state);
    let loopback = cfg.bind == "127.0.0.1" || cfg.bind == "localhost" || cfg.bind == "::1";
    let admin = AdminState {
        data_dir: data_dir.to_path_buf(),
        config: Arc::new(std::sync::Mutex::new(cfg.clone())),
        loopback,
        traffic,
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

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find_map(|w| {
        if w[0] == name {
            Some(w[1].clone())
        } else {
            None
        }
    })
}

fn pick_clients(args: &[String]) -> Result<Vec<&'static str>, Box<dyn std::error::Error>> {
    if let Some(raw) = flag_value(args, "--clients") {
        return Ok(clients::resolve_ids(&raw)?);
    }
    let detected = clients::detected_ids();
    if args.iter().any(|a| a == "--yes" || a == "-y") || !io::stdin().is_terminal() {
        if detected.is_empty() {
            return Ok(vec!["claude"]);
        }
        return Ok(detected);
    }
    println!("Which apps should send traffic through Veil?");
    for (i, spec) in clients::CLIENTS.iter().enumerate() {
        let mark = if clients::is_detected(spec.id) {
            "found"
        } else {
            "     "
        };
        println!("  {}) [{:5}] {}", i + 1, mark, spec.label);
    }
    print!("Numbers, 'all', or Enter for all found: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
        if detected.is_empty() {
            return Ok(clients::CLIENTS.iter().map(|c| c.id).collect());
        }
        return Ok(detected);
    }
    if line.eq_ignore_ascii_case("all") {
        return Ok(clients::CLIENTS.iter().map(|c| c.id).collect());
    }
    let mut ids = Vec::new();
    for part in line.split(|c: char| c == ',' || c.is_whitespace()) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Ok(n) = part.parse::<usize>() {
            if let Some(spec) = clients::CLIENTS.get(n.saturating_sub(1)) {
                if !ids.contains(&spec.id) {
                    ids.push(spec.id);
                }
                continue;
            }
        }
        ids.extend(clients::resolve_ids(part)?);
    }
    Ok(ids)
}

fn cmd_setup(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let ids = pick_clients(args)?;
    if ids.is_empty() {
        println!("no clients selected");
        return Ok(());
    }
    let explicit = flag_value(args, "--upstream");
    let mut previous = explicit.clone();
    println!("proxy {DEFAULT_PROXY_URL}");
    for id in &ids {
        let r = clients::apply_client(id, DEFAULT_PROXY_URL)?;
        println!("  {}  {}", r.id, r.path.display());
        if previous.is_none() {
            previous = r.previous;
        }
    }
    let data_dir = default_data_dir();
    let mut cfg = Config::load(&data_dir)?;
    let chained = setup::apply_chained_upstream(&mut cfg, previous.as_deref());
    if chained {
        cfg.save(&data_dir)?;
        println!("  custom upstream: {}", cfg.anthropic_upstream);
    } else {
        println!("  upstream: official APIs (override with --upstream URL or console 上游)");
    }
    println!();
    println!("Restart the selected apps. API keys stay in the client; Veil pass-throughs them.");
    Ok(())
}

async fn cmd_update(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let check_only = args.iter().any(|a| a == "--check");
    let info = update::check().await?;
    println!("current {}  latest {}", info.current, info.latest);
    if !info.newer {
        println!("already up to date");
        return Ok(());
    }
    if check_only {
        println!("update available: {}", info.asset);
        return Ok(());
    }
    let exe = update::current_exe()?;
    let staged = update::staged_next_to(&exe);
    println!("downloading {}", info.asset);
    update::download_and_stage(&staged).await?;
    update::schedule_replace(&exe, &staged)?;
    println!("saved {}. a helper will swap the exe and restart in a few seconds — you can close this window.", info.latest);
    Ok(())
}
