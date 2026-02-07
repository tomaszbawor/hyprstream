use crate::paths;
use crate::{hs_debug, hs_error, hs_info};
use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

pub struct HyprIpc {
    sock: std::path::PathBuf,
}

pub trait Hypr {
    fn request_raw(&self, args: &str) -> Result<String>;

    fn request_json<T: DeserializeOwned>(&self, args: &str) -> Result<T>;
}

fn ok_reply(resp: &str) -> bool {
    resp.trim().eq_ignore_ascii_case("ok")
}

impl HyprIpc {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sock: paths::hypr_socket_path()?,
        })
    }

    fn send_to_socket(path: &Path, payload: &[u8]) -> Result<String> {
        let mut s = UnixStream::connect(path)
            .with_context(|| format!("connect hyprland socket: {}", path.display()))?;
        s.write_all(payload).context("write to hyprland socket")?;

        let mut out = Vec::new();
        s.read_to_end(&mut out).context("read hyprland socket")?;
        Ok(String::from_utf8_lossy(&out).to_string())
    }

    pub fn request_raw(&self, args: &str) -> Result<String> {
        let payload = format!("/{args}");
        hs_debug!("hyprctl >> {}", args);
        let resp = Self::send_to_socket(&self.sock, payload.as_bytes())?;
        hs_debug!("hyprctl << {}", resp.trim_end());
        Ok(resp)
    }

    pub fn request_json<T: DeserializeOwned>(&self, args: &str) -> Result<T> {
        let payload = format!("j/{args}");
        hs_debug!("hyprctl -j >> {}", args);
        let resp = Self::send_to_socket(&self.sock, payload.as_bytes())?;
        hs_debug!("hyprctl -j << {}", resp.trim_end());
        serde_json::from_str(&resp)
            .with_context(|| format!("parse hyprctl json for {args}: {resp}"))
    }
}

impl Hypr for HyprIpc {
    fn request_raw(&self, args: &str) -> Result<String> {
        HyprIpc::request_raw(self, args)
    }

    fn request_json<T: DeserializeOwned>(&self, args: &str) -> Result<T> {
        HyprIpc::request_json(self, args)
    }
}

#[derive(Debug, Deserialize)]
pub struct Monitor {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub monitor: String,
}

#[derive(Debug, Deserialize)]
pub struct ActiveWorkspace {
    pub name: String,
}

pub fn create_headless(ipc: &impl Hypr) -> Result<String> {
    let resp = ipc.request_raw("output create headless")?;
    if !ok_reply(&resp) {
        return Err(anyhow!("hyprctl output create headless: {resp}"));
    }

    let monitors: Vec<Monitor> = ipc.request_json("monitors all")?;
    let mut best_num: i32 = -1;
    let mut best: Option<String> = None;
    for m in monitors {
        if let Some(rest) = m.name.strip_prefix("HEADLESS-") {
            if let Ok(n) = rest.parse::<i32>() {
                if n > best_num {
                    best_num = n;
                    best = Some(m.name);
                }
            }
        }
    }

    let name = best.ok_or_else(|| anyhow!("could not find headless output after creation"))?;
    hs_info!("created headless output: {}", name);
    Ok(name)
}

pub fn remove_headless(ipc: &impl Hypr, name: &str) -> Result<()> {
    let resp = ipc.request_raw(&format!("output remove {name}"))?;
    if !ok_reply(&resp) {
        return Err(anyhow!("failed to remove {name}: {resp}"));
    }
    hs_info!("removed headless output: {}", name);
    Ok(())
}

pub fn mirror_headless_from(ipc: &impl Hypr, headless: &str, source: &str) -> Result<()> {
    let cmd = format!("keyword monitor {headless},preferred,auto,1,mirror,{source}");
    let resp = ipc.request_raw(&cmd)?;
    if !ok_reply(&resp) {
        return Err(anyhow!(
            "failed to mirror {} from {}: {}",
            headless,
            source,
            resp
        ));
    }
    hs_info!("mirroring: {} -> {}", headless, source);
    Ok(())
}

pub fn disable_mirror(ipc: &impl Hypr, headless: &str, resolution: &str) -> Result<()> {
    let cmd = format!("keyword monitor {headless},{resolution},-9999x0,1");
    let resp = ipc.request_raw(&cmd)?;
    if !ok_reply(&resp) {
        return Err(anyhow!("failed to park {} off-screen: {}", headless, resp));
    }
    hs_info!("mirror disabled on {} (off-screen)", headless);
    Ok(())
}

pub fn monitor_exists(ipc: &impl Hypr, name: &str) -> Result<bool> {
    let monitors: Vec<Monitor> = ipc.request_json("monitors all")?;
    Ok(monitors.into_iter().any(|m| m.name == name))
}

pub fn bind_workspace_to_monitor(ipc: &impl Hypr, workspace: &str, monitor: &str) -> Result<()> {
    let cmd = format!("keyword workspace {workspace},monitor:{monitor},default:true");
    let resp = ipc.request_raw(&cmd)?;
    if !ok_reply(&resp) {
        hs_error!(
            "failed to bind workspace {} -> monitor {}: {}",
            workspace,
            monitor,
            resp
        );
        return Err(anyhow!("bind workspace failed"));
    }
    hs_info!("bound workspace {} -> monitor {}", workspace, monitor);
    Ok(())
}

pub fn move_workspace_to_monitor(ipc: &impl Hypr, workspace: &str, monitor: &str) -> Result<()> {
    let cmd = format!("dispatch moveworkspacetomonitor {workspace} {monitor}");
    let resp = ipc.request_raw(&cmd)?;
    if !ok_reply(&resp) {
        hs_error!(
            "failed to move workspace {} -> monitor {}: {}",
            workspace,
            monitor,
            resp
        );
        return Err(anyhow!("move workspace failed"));
    }
    hs_info!("moved workspace {} -> monitor {}", workspace, monitor);
    Ok(())
}

pub fn detect_physical_monitor(ipc: &impl Hypr) -> Result<String> {
    let monitors: Vec<Monitor> = ipc.request_json("monitors")?;
    for m in monitors {
        if !m.name.starts_with("HEADLESS-") {
            hs_info!("detected physical monitor: {}", m.name);
            return Ok(m.name);
        }
    }
    Err(anyhow!("no physical monitor found"))
}

pub fn active_workspace(ipc: &impl Hypr) -> Result<String> {
    let ws: ActiveWorkspace = ipc.request_json("activeworkspace")?;
    Ok(ws.name)
}

pub fn snapshot_workspaces(ipc: &impl Hypr) -> Result<HashMap<String, String>> {
    let workspaces: Vec<Workspace> = ipc.request_json("workspaces")?;
    Ok(workspaces
        .into_iter()
        .map(|w| (w.name, w.monitor))
        .collect())
}

pub fn restore_headless_stolen_workspaces(
    ipc: &impl Hypr,
    before: &HashMap<String, String>,
    headless: &str,
    streaming_workspace: &str,
) -> Result<usize> {
    let after = snapshot_workspaces(ipc)?;
    let mut moved = 0;

    for (ws, mon) in after.iter() {
        if mon != headless {
            continue;
        }
        if ws == streaming_workspace {
            continue;
        }
        let Some(prev) = before.get(ws) else { continue };
        if prev == headless {
            continue;
        }
        if prev.starts_with("HEADLESS-") {
            continue;
        }

        crate::hs_warn!(
            "workspace {} moved to {} during enable; restoring to {}",
            ws,
            headless,
            prev
        );
        move_workspace_to_monitor(ipc, ws, prev)?;
        moved += 1;
    }

    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    struct FakeHypr {
        // workspace -> monitor
        workspaces: RefCell<HashMap<String, String>>,
        moves: RefCell<Vec<(String, String)>>,
    }

    impl FakeHypr {
        fn new(workspaces: impl IntoIterator<Item = (String, String)>) -> Self {
            Self {
                workspaces: RefCell::new(workspaces.into_iter().collect()),
                moves: RefCell::new(Vec::new()),
            }
        }
    }

    impl Hypr for FakeHypr {
        fn request_raw(&self, args: &str) -> Result<String> {
            // Only implement what restore logic uses.
            if let Some(rest) = args.strip_prefix("dispatch moveworkspacetomonitor ") {
                let mut parts = rest.split_whitespace();
                let ws = parts.next().unwrap_or("");
                let mon = parts.next().unwrap_or("");
                self.workspaces
                    .borrow_mut()
                    .insert(ws.to_string(), mon.to_string());
                self.moves
                    .borrow_mut()
                    .push((ws.to_string(), mon.to_string()));
                return Ok("ok".to_string());
            }
            Ok("ok".to_string())
        }

        fn request_json<T: DeserializeOwned>(&self, args: &str) -> Result<T> {
            if args == "workspaces" {
                let list = self
                    .workspaces
                    .borrow()
                    .iter()
                    .map(|(name, monitor)| json!({"name": name, "monitor": monitor}))
                    .collect::<Vec<_>>();
                return Ok(serde_json::from_value(json!(list))?);
            }
            Err(anyhow!("unsupported json query in test: {args}"))
        }
    }

    #[test]
    fn restore_moves_only_non_streaming_workspaces_off_headless() {
        let before = HashMap::from([
            ("1".to_string(), "eDP-1".to_string()),
            ("9".to_string(), "eDP-1".to_string()),
        ]);
        let fake = FakeHypr::new([
            ("1".to_string(), "HEADLESS-1".to_string()),
            ("9".to_string(), "HEADLESS-1".to_string()),
        ]);

        let moved = restore_headless_stolen_workspaces(&fake, &before, "HEADLESS-1", "9").unwrap();
        assert_eq!(moved, 1);

        let moves = fake.moves.borrow();
        assert_eq!(moves.as_slice(), &[("1".to_string(), "eDP-1".to_string())]);
    }

    #[test]
    fn restore_does_not_move_workspaces_without_previous_mapping() {
        let before = HashMap::new();
        let fake = FakeHypr::new([("1".to_string(), "HEADLESS-1".to_string())]);

        let moved = restore_headless_stolen_workspaces(&fake, &before, "HEADLESS-1", "9").unwrap();
        assert_eq!(moved, 0);
    }
}
