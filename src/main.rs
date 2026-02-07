use std::env;
use std::process::ExitCode;

mod config;
mod control;
mod daemon;
mod hypr;
mod logging;
mod paths;

const VERSION: &str = "0.1.0";

fn usage(prog: &str) {
    eprintln!(
        "hyprstream {ver} - virtual output manager for Hyprland streaming\n\n\
Usage: {prog} <command> [options]\n\n\
Commands:\n\
  daemon [-c config]   Start the daemon\n\
  enable               Enable streaming mode\n\
  disable              Disable streaming mode\n\
  toggle               Toggle streaming mode\n\
  status               Show current status\n\
  version              Show version\n\n\
Config: ~/.config/hyprstream/config\n",
        ver = VERSION,
        prog = prog
    );
}

fn main() -> ExitCode {
    let mut args = env::args();
    let prog = args.next().unwrap_or_else(|| "hyprstream".to_string());
    let cmd = match args.next() {
        Some(c) => c,
        None => {
            usage(&prog);
            return ExitCode::from(1);
        }
    };

    if cmd == "version" || cmd == "--version" || cmd == "-v" {
        println!("hyprstream {VERSION}");
        return ExitCode::SUCCESS;
    }

    if cmd == "--help" || cmd == "-h" || cmd == "help" {
        usage(&prog);
        return ExitCode::SUCCESS;
    }

    if cmd == "daemon" {
        let mut config_path: Option<String> = None;

        while let Some(a) = args.next() {
            if a == "-c" || a == "--config" {
                match args.next() {
                    Some(p) => config_path = Some(p),
                    None => {
                        eprintln!("missing value for {a}");
                        return ExitCode::from(1);
                    }
                }
            } else if a == "-v" || a == "--verbose" {
                logging::set_level(logging::Level::Debug);
            } else {
                eprintln!("unknown option: {a}");
                return ExitCode::from(1);
            }
        }

        let cfg = match config::Config::load(config_path.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hyprstream: failed to load config: {e:#}");
                return ExitCode::from(1);
            }
        };

        match daemon::run(cfg) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("hyprstream: daemon error: {e:#}");
                ExitCode::from(1)
            }
        }
    } else if cmd == "enable" {
        if let Err(e) = control::send(control::CTL_ENABLE) {
            eprintln!("{e:#}");
            return ExitCode::from(1);
        }
        ExitCode::SUCCESS
    } else if cmd == "disable" {
        if let Err(e) = control::send(control::CTL_DISABLE) {
            eprintln!("{e:#}");
            return ExitCode::from(1);
        }
        ExitCode::SUCCESS
    } else if cmd == "toggle" {
        if let Err(e) = control::send(control::CTL_TOGGLE) {
            eprintln!("{e:#}");
            return ExitCode::from(1);
        }
        ExitCode::SUCCESS
    } else if cmd == "status" {
        if let Err(e) = control::send(control::CTL_STATUS) {
            eprintln!("{e:#}");
            return ExitCode::from(1);
        }
        ExitCode::SUCCESS
    } else {
        eprintln!("unknown command: {cmd}");
        usage(&prog);
        ExitCode::from(1)
    }
}
