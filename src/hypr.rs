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

pub fn create_headless(ipc: &HyprIpc) -> Result<String> {
    let resp = ipc.request_raw("output create headless")?;
    if !(resp.contains("ok") || resp.contains("Ok")) {
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

pub fn remove_headless(ipc: &HyprIpc, name: &str) -> Result<()> {
    let resp = ipc.request_raw(&format!("output remove {name}"))?;
    if !(resp.contains("ok") || resp.contains("Ok")) {
        return Err(anyhow!("failed to remove {name}: {resp}"));
    }
    hs_info!("removed headless output: {}", name);
    Ok(())
}

pub fn enable_mirror(ipc: &HyprIpc, headless: &str, physical: &str) -> Result<()> {
    let cmd = format!("keyword monitor {headless},preferred,auto,1,mirror,{physical}");
    let _ = ipc.request_raw(&cmd)?;
    hs_info!("mirroring: {} -> {}", headless, physical);
    Ok(())
}

pub fn disable_mirror(ipc: &HyprIpc, headless: &str, resolution: &str) -> Result<()> {
    let cmd = format!("keyword monitor {headless},{resolution},-9999x0,1");
    let _ = ipc.request_raw(&cmd)?;
    hs_info!("mirror disabled on {} (off-screen)", headless);
    Ok(())
}

pub fn bind_workspace_to_monitor(ipc: &HyprIpc, workspace: &str, monitor: &str) -> Result<()> {
    let cmd = format!("keyword workspace {workspace},monitor:{monitor},default:true");
    let resp = ipc.request_raw(&cmd)?;
    if !(resp.contains("ok") || resp.contains("Ok")) {
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

pub fn move_workspace_to_monitor(ipc: &HyprIpc, workspace: &str, monitor: &str) -> Result<()> {
    let cmd = format!("dispatch moveworkspacetomonitor {workspace} {monitor}");
    let resp = ipc.request_raw(&cmd)?;
    if !(resp.contains("ok") || resp.contains("Ok")) {
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

pub fn detect_physical_monitor(ipc: &HyprIpc) -> Result<String> {
    let monitors: Vec<Monitor> = ipc.request_json("monitors")?;
    for m in monitors {
        if !m.name.starts_with("HEADLESS-") {
            hs_info!("detected physical monitor: {}", m.name);
            return Ok(m.name);
        }
    }
    Err(anyhow!("no physical monitor found"))
}

pub fn active_workspace(ipc: &HyprIpc) -> Result<String> {
    let ws: ActiveWorkspace = ipc.request_json("activeworkspace")?;
    Ok(ws.name)
}

pub fn snapshot_workspaces(ipc: &HyprIpc) -> Result<HashMap<String, String>> {
    let workspaces: Vec<Workspace> = ipc.request_json("workspaces")?;
    Ok(workspaces
        .into_iter()
        .map(|w| (w.name, w.monitor))
        .collect())
}

pub fn restore_headless_stolen_workspaces(
    ipc: &HyprIpc,
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
