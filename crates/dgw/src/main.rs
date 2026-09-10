//! CLI: `dgw start [--foreground] | stop | status | ui`

use std::sync::Arc;

use dgw::admin::{self, AdminState, TrafficLog};
use dgw::config::Config;
use dgw::data_dir::default_data_dir;
use dgw::master_key::load_or_create;
use dgw::process::{self, StartOutcome};
use dgw::proxy::{router, AppState, UpstreamConfig};
use dgw_store::SqliteStore;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
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
        "ui" => cmd_ui(),
        _ => {
            eprintln!("usage: dgw <start|stop|status|ui> [--foreground]");
            std::process::exit(2);
        }
    }
}

async fn cmd_start(foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = default_data_dir();
    let cfg = Config::load(&data_dir)?;
    if !foreground {
        if let Some(existing) = process::read_pid(&data_dir)? {
            if process::pid_is_alive(existing.pid) && existing.exe == process::current_exe() {
                println!(
                    "already running proxy={} management={}",
                    existing.proxy_port, existing.management_port
                );
                return Ok(());
            }
        }
        process::spawn_daemon(&process::current_exe())?;
        println!(
            "starting proxy={} management={}",
            cfg.proxy_port, cfg.management_port
        );
        return Ok(());
    }
    let outcome = process::try_start(
        &data_dir,
        cfg.proxy_port,
        cfg.management_port,
        &process::current_exe(),
    )?;
    match outcome {
        StartOutcome::AlreadyRunning {
            proxy_port,
            management_port,
        } => {
            println!("already running proxy={proxy_port} management={management_port}");
            Ok(())
        }
        StartOutcome::Started {
            proxy_port,
            management_port,
        } => {
            println!("proxy={proxy_port} management={management_port}");
            serve(&data_dir, &cfg).await
        }
    }
}

async fn serve(data_dir: &std::path::Path, cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let limit = (cfg.request_body_limit_mib.saturating_mul(1024 * 1024)) as usize;
    let key = load_or_create(data_dir, cfg.mode)?;
    let store = SqliteStore::open(data_dir.join("mappings.db"), &key)
        .map_err(|e| format!("mapping store: {e}"))?;
    let state = AppState::new(
        Arc::new(store),
        UpstreamConfig::from(cfg),
        limit,
    );
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
                "running pid={} proxy={} management={}",
                p.pid, p.proxy_port, p.management_port
            );
        }
        Some(p) => println!("stale pid file pid={}", p.pid),
        None => println!("not running"),
    }
    Ok(())
}

fn cmd_ui() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = default_data_dir();
    let cfg = Config::load(&data_dir)?;
    println!("http://{}:{}/", cfg.bind, cfg.management_port);
    Ok(())
}
