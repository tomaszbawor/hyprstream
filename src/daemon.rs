use crate::config::Config;
use crate::control::Command as CtlCommand;
use crate::hypr;
use crate::paths;
use crate::{hs_error, hs_info, hs_warn};
use anyhow::{Context, Result};
use libc::{poll, pollfd, POLLERR, POLLHUP, POLLIN};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command as ProcessCommand;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

const RECONNECT_DELAY_SECS: u64 = 2;
const MAX_RECONNECT: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Disabled,
    Enabled,
}

struct State {
    mode: Mode,
    headless: Option<String>,
    physical: String,
    active_workspace: String,
    mirroring_active: bool,
    cfg: Config,
}

fn exec_async(cmd: &str) {
    if cmd.trim().is_empty() {
        return;
    }
    let _ = ProcessCommand::new("sh").arg("-c").arg(cmd).spawn();
}

#[cfg(test)]
mod tests {
    use super::{reconcile, Mode, State};
    use crate::config::Config;
    use crate::hypr::Hypr;
    use anyhow::{anyhow, Result};
    use serde::de::DeserializeOwned;
    use serde_json::json;
    use std::cell::RefCell;

    struct FakeHypr {
        active_ws: RefCell<String>,
        raw_calls: RefCell<Vec<String>>,
    }

    impl FakeHypr {
        fn new(active_ws: &str) -> Self {
            Self {
                active_ws: RefCell::new(active_ws.to_string()),
                raw_calls: RefCell::new(Vec::new()),
            }
        }

        fn set_active(&self, ws: &str) {
            *self.active_ws.borrow_mut() = ws.to_string();
        }
    }

    impl Hypr for FakeHypr {
        fn request_raw(&self, args: &str) -> Result<String> {
            self.raw_calls.borrow_mut().push(args.to_string());
            Ok("ok".to_string())
        }

        fn request_json<T: DeserializeOwned>(&self, args: &str) -> Result<T> {
            if args == "activeworkspace" {
                let name = self.active_ws.borrow().clone();
                let v = json!({"name": name});
                return Ok(serde_json::from_value::<T>(v)?);
            }
            Err(anyhow!("unsupported json query in test: {args}"))
        }
    }

    #[test]
    fn reconcile_enables_and_disables_mirror_on_workspace_change() {
        let cfg = Config::default();
        let fake = FakeHypr::new(&cfg.streaming_workspace);
        let mut st = State {
            mode: Mode::Enabled,
            headless: Some("HEADLESS-1".to_string()),
            physical: "eDP-1".to_string(),
            active_workspace: String::new(),
            mirroring_active: false,
            cfg,
        };

        reconcile(&mut st, &fake).unwrap();
        assert!(st.mirroring_active);
        assert!(fake
            .raw_calls
            .borrow()
            .iter()
            .any(|c| c.contains("mirror,eDP-1")));

        fake.set_active("1");
        reconcile(&mut st, &fake).unwrap();
        assert!(!st.mirroring_active);
        assert!(fake
            .raw_calls
            .borrow()
            .iter()
            .any(|c| c.contains("-9999x0")));
    }
}

fn reconcile(st: &mut State, ipc: &impl hypr::Hypr) -> Result<()> {
    if st.mode != Mode::Enabled {
        return Ok(());
    }
    let headless = match st.headless.as_deref() {
        Some(h) => h,
        None => return Ok(()),
    };

    let ws = hypr::active_workspace(ipc)?;
    let on_stream = ws == st.cfg.streaming_workspace;

    if on_stream && !st.mirroring_active {
        hs_info!("entering streaming workspace -> enable mirror");
        hypr::enable_mirror(ipc, headless, &st.physical)?;
        st.mirroring_active = true;
        if !st.cfg.on_streaming_enter.is_empty() {
            exec_async(&st.cfg.on_streaming_enter);
        }
    } else if !on_stream && st.mirroring_active {
        hs_info!("leaving streaming workspace -> disable mirror");
        hypr::disable_mirror(ipc, headless, &st.cfg.virtual_resolution)?;
        st.mirroring_active = false;
        if !st.cfg.on_streaming_leave.is_empty() {
            exec_async(&st.cfg.on_streaming_leave);
        }
    }

    st.active_workspace = ws;
    Ok(())
}

fn streaming_enable(st: &mut State, ipc: &impl hypr::Hypr) -> Result<()> {
    if st.mode == Mode::Enabled {
        hs_info!("already enabled");
        return Ok(());
    }

    if st.physical.is_empty() {
        st.physical = if !st.cfg.physical_monitor.is_empty() {
            st.cfg.physical_monitor.clone()
        } else {
            hypr::detect_physical_monitor(ipc)?
        };
    }

    let before: HashMap<String, String> = hypr::snapshot_workspaces(ipc)?;
    let headless = hypr::create_headless(ipc)?;

    // park headless off-screen
    hypr::disable_mirror(ipc, &headless, &st.cfg.virtual_resolution)?;

    // bind/move streaming workspace to headless
    let res = (|| -> Result<()> {
        hypr::bind_workspace_to_monitor(ipc, &st.cfg.streaming_workspace, &headless)?;
        hypr::move_workspace_to_monitor(ipc, &st.cfg.streaming_workspace, &headless)?;
        Ok(())
    })();

    if let Err(e) = res {
        let _ = hypr::restore_headless_stolen_workspaces(
            ipc,
            &before,
            &headless,
            &st.cfg.streaming_workspace,
        );
        let _ = hypr::remove_headless(ipc, &headless);
        return Err(e);
    }

    let _ = hypr::restore_headless_stolen_workspaces(
        ipc,
        &before,
        &headless,
        &st.cfg.streaming_workspace,
    );

    st.mode = Mode::Enabled;
    st.headless = Some(headless);
    st.mirroring_active = false;
    st.active_workspace.clear();

    reconcile(st, ipc)?;

    if !st.cfg.on_enable.is_empty() {
        exec_async(&st.cfg.on_enable);
    }

    hs_info!(
        "streaming mode enabled (headless={:?}, physical={})",
        st.headless,
        st.physical
    );
    Ok(())
}

fn streaming_disable(st: &mut State, ipc: &impl hypr::Hypr) -> Result<()> {
    if st.mode == Mode::Disabled {
        hs_info!("already disabled");
        return Ok(());
    }

    if let Some(headless) = st.headless.as_deref() {
        if st.mirroring_active {
            hypr::disable_mirror(ipc, headless, &st.cfg.virtual_resolution)?;
            st.mirroring_active = false;
        }

        let _ = hypr::move_workspace_to_monitor(ipc, &st.cfg.streaming_workspace, &st.physical);
        let _ = hypr::remove_headless(ipc, headless);
    }

    st.headless = None;
    st.mode = Mode::Disabled;

    if !st.cfg.on_disable.is_empty() {
        exec_async(&st.cfg.on_disable);
    }

    hs_info!("streaming mode disabled");
    Ok(())
}

fn handle_ctl_with_running(
    client: UnixStream,
    st: &mut State,
    ipc: &impl hypr::Hypr,
    running: &Arc<AtomicBool>,
) -> Result<()> {
    // Peek command first so we can flip running for quit.
    let mut buf = [0u8; 256];
    let mut client = client;
    let n = client.read(&mut buf).context("read control command")?;
    if n == 0 {
        return Ok(());
    }
    let cmd_raw = String::from_utf8_lossy(&buf[..n]).trim().to_string();

    let response = match CtlCommand::parse(&cmd_raw) {
        Ok(CtlCommand::Enable) => {
            if streaming_enable(st, ipc).is_ok() {
                "enabled".to_string()
            } else {
                "error: enable failed".to_string()
            }
        }
        Ok(CtlCommand::Disable) => {
            if streaming_disable(st, ipc).is_ok() {
                "disabled".to_string()
            } else {
                "error: disable failed".to_string()
            }
        }
        Ok(CtlCommand::Toggle) => {
            let rc = if st.mode == Mode::Enabled {
                streaming_disable(st, ipc)
            } else {
                streaming_enable(st, ipc)
            };
            if rc.is_ok() {
                if st.mode == Mode::Enabled {
                    "enabled".to_string()
                } else {
                    "disabled".to_string()
                }
            } else {
                "error: toggle failed".to_string()
            }
        }
        Ok(CtlCommand::Status) => format!(
            "mode={} headless={} physical={} workspace={} mirroring={}",
            if st.mode == Mode::Enabled {
                "enabled"
            } else {
                "disabled"
            },
            st.headless.as_deref().unwrap_or("none"),
            if st.physical.is_empty() {
                "unknown"
            } else {
                &st.physical
            },
            st.active_workspace,
            if st.mirroring_active { "on" } else { "off" }
        ),
        Ok(CtlCommand::Quit) => "shutting down".to_string(),
        Err(_e) => format!("error: unknown command: {cmd_raw}"),
    };

    if cmd_raw.trim() == CtlCommand::Quit.as_str() {
        running.store(false, Ordering::Relaxed);
    }
    let _ = client.write_all(response.as_bytes());
    Ok(())
}

fn create_ctl_listener() -> Result<UnixListener> {
    let path = paths::ctl_socket_path();
    let _ = fs::remove_file(&path);
    let l = UnixListener::bind(&path)
        .with_context(|| format!("bind control socket: {}", path.display()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    hs_info!("control socket: {}", path.display());
    Ok(l)
}

fn connect_events() -> Result<UnixStream> {
    let p = paths::hypr_event_socket_path()?;
    let s = UnixStream::connect(&p)
        .with_context(|| format!("connect hyprland IPC: {}", p.display()))?;
    hs_info!("connected to hyprland IPC: {}", p.display());
    Ok(s)
}

fn handle_event_line(st: &mut State, ipc: &impl hypr::Hypr, line: &str) {
    let Some((event, data)) = line.split_once(">>") else {
        return;
    };

    match event {
        "workspace" | "focusedmon" | "activewindow" | "movewindow" => {
            if let Err(e) = reconcile(st, ipc) {
                hs_warn!("reconcile failed: {e:#}");
            }
        }
        "monitorremoved" => {
            if st.mode == Mode::Enabled {
                if let Some(headless) = st.headless.as_deref() {
                    if data == headless {
                        hs_warn!("headless output was removed externally");
                        st.mode = Mode::Disabled;
                        st.headless = None;
                        st.mirroring_active = false;
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn run(cfg: Config) -> Result<()> {
    let ipc = hypr::HyprIpc::new()?;

    let running = Arc::new(AtomicBool::new(true));
    // Best-effort signal handling; if it fails we still run.
    let _ = signal_hook::flag::register(libc::SIGINT, Arc::clone(&running));
    let _ = signal_hook::flag::register(libc::SIGTERM, Arc::clone(&running));

    let ctl = create_ctl_listener()?;
    ctl.set_nonblocking(true)?;

    let mut st = State {
        mode: Mode::Disabled,
        headless: None,
        physical: String::new(),
        active_workspace: String::new(),
        mirroring_active: false,
        cfg,
    };

    if st.cfg.auto_enable {
        if let Err(e) = streaming_enable(&mut st, &ipc) {
            hs_warn!("auto-enable failed, continuing in disabled mode: {e:#}");
        }
    }

    let mut reconnect_attempts = 0usize;
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);

    while running.load(Ordering::Relaxed) {
        let mut events = match connect_events() {
            Ok(s) => s,
            Err(e) => {
                reconnect_attempts += 1;
                if reconnect_attempts > MAX_RECONNECT {
                    hs_error!("max reconnect attempts reached, exiting");
                    break;
                }
                hs_warn!(
                    "IPC connect failed, retrying in {}s ({}/{}) ({e:#})",
                    RECONNECT_DELAY_SECS,
                    reconnect_attempts,
                    MAX_RECONNECT
                );
                std::thread::sleep(Duration::from_secs(RECONNECT_DELAY_SECS));
                continue;
            }
        };
        reconnect_attempts = 0;
        events.set_nonblocking(true)?;

        let mut fds = [
            pollfd {
                fd: events.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
            pollfd {
                fd: ctl.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        'inner: loop {
            if !running.load(Ordering::Relaxed) {
                break 'inner;
            }
            let rc = unsafe { poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 1000) };
            if rc < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e).context("poll");
            }

            if (fds[0].revents & (POLLHUP | POLLERR)) != 0 {
                hs_warn!("IPC socket error, reconnecting...");
                break 'inner;
            }

            if (fds[0].revents & POLLIN) != 0 {
                let mut tmp = [0u8; 65536];
                match events.read(&mut tmp) {
                    Ok(0) => {
                        hs_warn!("IPC connection lost, reconnecting...");
                        break 'inner;
                    }
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                            let line = buf.drain(..=pos).collect::<Vec<u8>>();
                            let line = String::from_utf8_lossy(&line);
                            let line = line.trim_end_matches(['\n', '\r']);
                            if !line.is_empty() {
                                handle_event_line(&mut st, &ipc, line);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        hs_warn!("IPC read error, reconnecting: {}", e);
                        break 'inner;
                    }
                }
            }

            if (fds[1].revents & POLLIN) != 0 {
                loop {
                    match ctl.accept() {
                        Ok((client, _addr)) => {
                            if let Err(e) = handle_ctl_with_running(client, &mut st, &ipc, &running)
                            {
                                hs_warn!("control handler failed: {e:#}");
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e).context("accept control")?,
                    }
                }
            }
        }
    }

    if st.mode == Mode::Enabled {
        let _ = streaming_disable(&mut st, &ipc);
    }
    let _ = fs::remove_file(paths::ctl_socket_path());
    hs_info!("daemon stopped");
    Ok(())
}
