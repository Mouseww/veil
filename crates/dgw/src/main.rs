//! CLI: `dgw` (no args) starts and opens the UI. Also: start | stop | status | setup | ui

use std::sync::Arc;

use dgw::admin::{self, AdminState, TrafficLog};
use dgw::config::Config;
use dgw::data_dir::default_data_dir;
use dgw::master_key::load_or_create;
use dgw::process::{self, StartOutcome};
use dgw::proxy::{router, AppState, UpstreamConfig};
use dgw::setup::{self, DEFAULT_PROXY_URL};
use dgw_store::SqliteStore;
use tokio::net::TcpListener;

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
    eprintln!("Veil (dgw) — 本地 AI 脱敏代理");
    eprintln!("");
    eprintln!("  双击 dgw.exe，或运行:  dgw");
    eprintln!("  会启动网关并打开浏览器。然后执行:  dgw setup");
    eprintln!("  即可让 Claude Code 走本地脱敏。");
    eprintln!("");
    eprintln!("  dgw start          后台启动");
    eprintln!("  dgw start --foreground");
    eprintln!("  dgw setup          一键写入 Claude Code 配置");
    eprintln!("  dgw ui             打开管理页");
    eprintln!("  dgw status");
    eprintln!("  dgw stop");
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
    println!("Veil 已启动");
    println!("  管理页: {ui}");
    println!("  代理:   {DEFAULT_PROXY_URL}");
    println!("");
    println!("下一步（二选一）:");
    println!("  1) 运行  dgw setup     （一键配置 Claude Code）");
    println!("  2) 自己设置环境变量 ANTHROPIC_BASE_URL={DEFAULT_PROXY_URL}");
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
    println!("已停止");
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    match process::read_pid(&default_data_dir())? {
        Some(p) if process::pid_is_alive(p.pid) => {
            println!(
                "运行中 pid={} 代理={} 管理={}",
                p.pid, p.proxy_port, p.management_port
            );
        }
        Some(p) => println!("pid 文件过期 pid={}", p.pid),
        None => println!("未运行。执行 dgw 即可启动。"),
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
    println!("已写入 Claude Code 用户配置:");
    println!("  {}", path.display());
    println!("  ANTHROPIC_BASE_URL={DEFAULT_PROXY_URL}");
    if let Some(p) = prev {
        println!("  原上游已记下: {p}");
        println!("  （在管理页「上游」里可看到/修改）");
    }
    println!("");
    println!("请重启 Claude Code。登录态不用动。");
    Ok(())
}
