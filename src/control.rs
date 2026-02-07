use crate::paths;
use anyhow::{anyhow, Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Enable,
    Disable,
    Toggle,
    Status,
    Quit,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::Enable => "enable",
            Command::Disable => "disable",
            Command::Toggle => "toggle",
            Command::Status => "status",
            Command::Quit => "quit",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim() {
            "enable" => Ok(Command::Enable),
            "disable" => Ok(Command::Disable),
            "toggle" => Ok(Command::Toggle),
            "status" => Ok(Command::Status),
            "quit" => Ok(Command::Quit),
            other => Err(anyhow!("unknown command: {other}")),
        }
    }
}

pub fn send(cmd: Command) -> Result<()> {
    let path = paths::ctl_socket_path();

    let mut s = UnixStream::connect(&path).with_context(|| {
        format!(
            "hyprstream: daemon not running (connect {})",
            path.display()
        )
    })?;

    s.write_all(cmd.as_str().as_bytes())
        .with_context(|| format!("write control command: {}", cmd.as_str()))?;

    let mut buf = Vec::new();
    s.read_to_end(&mut buf).context("read control response")?;

    if !buf.is_empty() {
        let out = String::from_utf8_lossy(&buf);
        println!("{}", out.trim_end());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Command;

    #[test]
    fn parse_command_trims_whitespace() {
        assert_eq!(Command::parse(" enable ").unwrap(), Command::Enable);
        assert_eq!(Command::parse("status\n").unwrap(), Command::Status);
    }

    #[test]
    fn parse_command_rejects_unknown() {
        let err = Command::parse("nope").unwrap_err();
        assert!(err.to_string().contains("unknown command"));
    }
}
