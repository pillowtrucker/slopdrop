use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub tcl: TclConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub hostname: String,
    pub port: u16,
    pub use_tls: bool,
    pub nickname: String,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub privileged_users: Vec<String>,
    pub eval_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TclConfig {
    pub state_path: PathBuf,
    pub max_output_lines: usize,
    /// Optional remote git repository URL to clone state from
    /// If set and state_path doesn't exist, will clone from this URL
    /// Example: "https://github.com/user/bot-state.git"
    pub state_repo: Option<String>,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}
