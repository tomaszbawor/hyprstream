use crate::paths;
use crate::{hs_info, hs_warn};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub streaming_workspace: String,
    pub physical_monitor: String,
    pub virtual_resolution: String,
    pub on_streaming_enter: String,
    pub on_streaming_leave: String,
    pub on_enable: String,
    pub on_disable: String,
    pub auto_enable: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            streaming_workspace: "9".to_string(),
            physical_monitor: String::new(),
            virtual_resolution: "1920x1080@60".to_string(),
            on_streaming_enter: String::new(),
            on_streaming_leave: String::new(),
            on_enable: String::new(),
            on_disable: String::new(),
            auto_enable: false,
        }
    }
}

impl Config {
    pub fn load(path: Option<&str>) -> Result<Self> {
        let cfg = Self::default();
        let path = path
            .map(|p| p.to_string())
            .unwrap_or_else(|| paths::config_default_path().to_string_lossy().to_string());

        let p = Path::new(&path);
        let content = match fs::read_to_string(p) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                hs_info!("config file not found: {} (using defaults)", path);
                return Ok(cfg);
            }
            Err(e) => return Err(e).with_context(|| format!("read config: {path}")),
        };

        hs_info!("loading config: {}", path);
        Ok(Self::parse_with_defaults(cfg, &content))
    }

    fn parse_with_defaults(mut cfg: Config, content: &str) -> Config {
        for (idx, raw) in content.lines().enumerate() {
            let lineno = idx + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                hs_warn!("config:{}: malformed line (missing '=')", lineno);
                continue;
            };
            let key = k.trim();
            let val = v.trim();

            match key {
                "streaming_workspace" => cfg.streaming_workspace = val.to_string(),
                "physical_monitor" => cfg.physical_monitor = val.to_string(),
                "virtual_resolution" => cfg.virtual_resolution = val.to_string(),
                "on_streaming_enter" => cfg.on_streaming_enter = val.to_string(),
                "on_streaming_leave" => cfg.on_streaming_leave = val.to_string(),
                "on_enable" => cfg.on_enable = val.to_string(),
                "on_disable" => cfg.on_disable = val.to_string(),
                "auto_enable" => cfg.auto_enable = val == "true" || val == "1",
                _ => hs_warn!("unknown config key: {}", key),
            }
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parse_preserves_defaults_when_empty() {
        let cfg = Config::parse_with_defaults(Config::default(), "\n# comment\n\n");
        assert_eq!(cfg.streaming_workspace, "9");
        assert_eq!(cfg.virtual_resolution, "1920x1080@60");
        assert!(!cfg.auto_enable);
    }

    #[test]
    fn parse_trims_and_sets_values() {
        let cfg = Config::parse_with_defaults(
            Config::default(),
            " streaming_workspace =  12\nvirtual_resolution=1280x720@60\nauto_enable = true\n",
        );
        assert_eq!(cfg.streaming_workspace, "12");
        assert_eq!(cfg.virtual_resolution, "1280x720@60");
        assert!(cfg.auto_enable);
    }

    #[test]
    fn parse_ignores_malformed_lines() {
        let cfg =
            Config::parse_with_defaults(Config::default(), "noeqhere\nstreaming_workspace=7\n");
        assert_eq!(cfg.streaming_workspace, "7");
    }
}
