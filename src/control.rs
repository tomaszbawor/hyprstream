use crate::paths;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

pub const CTL_ENABLE: &str = "enable";
pub const CTL_DISABLE: &str = "disable";
pub const CTL_TOGGLE: &str = "toggle";
pub const CTL_STATUS: &str = "status";
pub const CTL_QUIT: &str = "quit";

pub fn send(cmd: &str) -> Result<()> {
    let path = paths::ctl_socket_path();

    let mut s = UnixStream::connect(&path).with_context(|| {
        format!(
            "hyprstream: daemon not running (connect {})",
            path.display()
        )
    })?;

    s.write_all(cmd.as_bytes())
        .with_context(|| format!("write control command: {cmd}"))?;

    let mut buf = Vec::new();
    s.read_to_end(&mut buf).context("read control response")?;

    if !buf.is_empty() {
        let out = String::from_utf8_lossy(&buf);
        println!("{}", out.trim_end());
    }

    Ok(())
}
