//! CLI: `dgw start [--foreground] | stop | status | ui`
//!
//! `start --foreground` currently loads config, writes the pid file, prints
//! ports, and exits. The reverse-proxy server is Task 11.

use dgw::config::Config;
use dgw::data_dir::default_data_dir;
use dgw::process::{self, StartOutcome};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let foreground = args.iter().any(|a| a == "--foreground");
    if let Err(e) = dispatch(cmd, foreground) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn dispatch(cmd: &str, foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "start" => cmd_start(foreground),
        "stop" => cmd_stop(),
        "status" => cmd_status(),
        "ui" => cmd_ui(),
        _ => {
            eprintln!("usage: dgw <start|stop|status|ui> [--foreground]");
            std::process::exit(2);
        }
    }
}

fn cmd_start(foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = default_data_dir();
    let cfg = Config::load(&data_dir)?;
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
        }
        StartOutcome::Started {
            proxy_port,
            management_port,
        } => {
            println!("proxy={proxy_port} management={management_port}");
            // Task 10: pid is written and we return immediately so cargo test
            // never hangs. The real server loop is Task 11.
            if foreground {
                eprintln!("--foreground wrote pid and is exiting (server is Task 11)");
            }
        }
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
