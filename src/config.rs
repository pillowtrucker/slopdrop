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
    /// Memory limit per evaluation in megabytes (Unix only, 0 = no limit)
    /// Default: 256 MB
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u64,
    /// Maximum recursion depth for TCL procedures (0 = no limit)
    /// Default: 1000
    #[serde(default = "default_recursion_limit")]
    pub max_recursion_depth: u32,
}

fn default_memory_limit() -> u64 {
    256 // 256 MB default
}

fn default_recursion_limit() -> u32 {
    1000 // 1000 levels deep
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TclConfig {
    pub state_path: PathBuf,
    pub max_output_lines: usize,
    /// Optional remote git repository URL to clone state from
    /// If set and state_path doesn't exist, will clone from this URL
    /// Example: "https://github.com/user/bot-state.git"
    pub state_repo: Option<String>,
    /// Optional SSH private key path for git push authentication
    /// Required if using SSH URLs (git@github.com:user/repo.git)
    /// Example: "/home/user/.ssh/id_rsa"
    pub ssh_key: Option<PathBuf>,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}
