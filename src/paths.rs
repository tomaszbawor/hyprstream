use anyhow::{anyhow, Result};
use std::env;
use std::path::{Path, PathBuf};

pub fn config_default_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Path::new(&xdg).join("hyprstream/config");
        }
    }
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return Path::new(&home).join(".config/hyprstream/config");
        }
    }
    PathBuf::from("/etc/hyprstream/config")
}

pub fn ctl_socket_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        if !xdg.is_empty() {
            return Path::new(&xdg).join("hyprstream.sock");
        }
    }
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/hyprstream-{uid}.sock"))
}

pub fn hypr_socket_path() -> Result<PathBuf> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if sig.is_empty() || xdg.is_empty() {
        return Err(anyhow!(
            "HYPRLAND_INSTANCE_SIGNATURE or XDG_RUNTIME_DIR not set"
        ));
    }
    Ok(Path::new(&xdg).join(format!("hypr/{sig}/.socket.sock")))
}

pub fn hypr_event_socket_path() -> Result<PathBuf> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if sig.is_empty() || xdg.is_empty() {
        return Err(anyhow!(
            "HYPRLAND_INSTANCE_SIGNATURE or XDG_RUNTIME_DIR not set"
        ));
    }
    Ok(Path::new(&xdg).join(format!("hypr/{sig}/.socket2.sock")))
}
