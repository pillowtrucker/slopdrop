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
    /// `[veles]` — the inference bridge: a native `ai` Tcl command that
    /// asks a veles agent over A2A. Absent means the command is not
    /// registered at all.
    #[serde(default)]
    pub veles: Option<crate::veles_bridge::VelesConfig>,
    /// `[web]` — the HTTP API's bind and its tokens. Absent means the
    /// built-in defaults (loopback, port 8080, no tokens), which is what
    /// `--web` did before this section existed.
    #[serde(default)]
    pub web: Option<WebConfigFile>,
}

/// The `[web]` section.
///
/// Until 2026-09-11 the web frontend was constructed from
/// `WebConfig::default()` with no way to configure it at all: that meant
/// `--web` served an eval API on 127.0.0.1:8080 with authentication
/// switched off and `is_admin` taken from the REQUEST BODY — so any
/// local process could ask for, and get, the unrestricted interpreter.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WebConfigFile {
    /// Default `127.0.0.1`. Binding anywhere else REQUIRES at least one
    /// token — the same rule veles' own HTTP surfaces follow: no
    /// credential means loopback only, and a credential alone does not
    /// widen the bind, it merely permits you to.
    #[serde(default)]
    pub bind_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    /// Bearer tokens, each with its own privilege. EMPTY means no
    /// authentication, which is only allowed on loopback.
    #[serde(default)]
    pub tokens: Vec<WebToken>,
}

/// One bearer token and what it may do.
///
/// Privilege belongs to the TOKEN, not to a flag in the request body.
/// The body's `is_admin` was self-service: whoever could reach the
/// endpoint could ask for `tclAdmin` — exec, file, socket — and get it.
/// A caller now gets admin because the operator issued them an admin
/// token, which is a decision made once in a config file instead of
/// per-request by the caller.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebToken {
    pub token: String,
    /// May this token run the unrestricted interpreter and roll state
    /// back? Default false — the safe interp, like an IRC `tcl` line.
    #[serde(default)]
    pub admin: bool,
    /// A label for logs and for the git author when the caller names
    /// nobody. Optional.
    #[serde(default)]
    pub name: Option<String>,
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
    /// Whether to include the full TCL stack trace (errorInfo) in error
    /// messages reported to the channel. When false, only the error body is
    /// shown; users can inspect the full trace via `tcl puts $errorInfo`.
    /// Default: false
    #[serde(default)]
    pub show_error_traces: bool,
}
