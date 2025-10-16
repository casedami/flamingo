pub mod git;
pub mod path;
mod process;

use crate::git::GitConfig;
use crate::path::PathConfig;
use crate::process::ProcessConfig;
use flamingo_err::ConfigError;

use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

fn get_config_path() -> Result<OsString, ConfigError> {
    let flamingo_cfg = "flamingo.toml";
    let flamingo_dir = "flamingo";

    // 1. XDG_CONFIG_HOME
    if let Ok(xdg_cfg) = env::var("XDG_CONFIG_HOME") {
        let cfg = PathBuf::from(xdg_cfg).join(flamingo_dir).join(flamingo_cfg);
        if cfg.exists() {
            return Ok(cfg.into());
        }
    }

    // 2. $HOME/.config
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".config").join(flamingo_dir).join(flamingo_cfg);
        if cfg.exists() {
            return Ok(cfg.into());
        }
    }

    Err(ConfigError::NotFound)
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct FlamingoConfig {
    pub path: PathConfig,
    pub git: GitConfig,
    #[serde(default)]
    pub process: ProcessConfig,
}

impl FlamingoConfig {
    pub fn load() -> Result<FlamingoConfig, ConfigError> {
        let cfg_path = get_config_path()?;
        let content = fs::read_to_string(cfg_path).map_err(ConfigError::IoError)?;
        let config: FlamingoConfig = toml::from_str(&content).map_err(ConfigError::ParseError)?;
        Ok(config)
    }

    pub fn setup_logger(&self) {
        flamingo_logger::init(
            &self.process.log_dir,
            &self.process.session_key,
            &self.process.min_log_level,
        );
    }
}
