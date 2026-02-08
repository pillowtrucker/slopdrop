use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Legacy single server config (backwards compatible with [server])
    #[serde(default)]
    pub server: Option<ServerConfig>,
    /// Multi-network server configs (use [[servers]] in TOML)
    #[serde(default)]
    pub servers: Option<Vec<ServerConfig>>,
    pub security: SecurityConfig,
    pub tcl: TclConfig,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Get the list of server configs, supporting both [server] and [[servers]] syntax
    pub fn get_servers(&self) -> Vec<ServerConfig> {
        if let Some(ref servers) = self.servers {
            if !servers.is_empty() {
                return servers.clone();
            }
        }
        if let Some(ref server) = self.server {
            return vec![server.clone()];
        }
        vec![]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Network name identifier (defaults to hostname if not set)
    #[serde(default)]
    pub name: Option<String>,
    pub hostname: String,
    pub port: u16,
    pub use_tls: bool,
    pub nickname: String,
    pub channels: Vec<String>,
}

impl ServerConfig {
    /// Get the network name, defaulting to hostname if not explicitly set
    pub fn network_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.hostname.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub privileged_users: Vec<String>,
    /// Blacklisted user hostmask patterns (denied from running eval commands)
    /// Example: ["baduser!*@*", "*!*@evil.example.com"]
    #[serde(default)]
    pub blacklisted_users: Vec<String>,
    pub eval_timeout_ms: u64,
    /// Memory limit per evaluation in megabytes (Unix only, 0 = no limit)
    /// Note: Uses RLIMIT_AS which limits entire process address space.
    /// Set to 0 (disabled) by default as small values cause crashes.
    /// If enabling, use values >= 1024 MB to account for process overhead.
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u64,
    /// Maximum recursion depth for TCL procedures (0 = no limit)
    /// Default: 1000
    #[serde(default = "default_recursion_limit")]
    pub max_recursion_depth: u32,
    /// Send commit notifications to the admin who made the change
    /// Default: false (only notify other admins)
    #[serde(default)]
    pub notify_self: bool,
}

fn default_memory_limit() -> u64 {
    0 // Disabled by default - RLIMIT_AS affects entire process, not just TCL thread
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
