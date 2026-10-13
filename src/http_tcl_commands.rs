/// TCL code that implements HTTP commands with rate limiting
/// This uses TCL's built-in http package (safe within our timeout protection)
///
/// TCL script is stored in tcl/http.tcl and loaded at runtime for hot-reloading

use std::fs;
use std::path::PathBuf;
use tracing::warn;

/// Get the TCL directory path relative to current working directory or executable
fn get_tcl_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    let tcl_dir = cwd.join("tcl");

    if tcl_dir.exists() {
        return tcl_dir;
    }

    // Fallback to executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let tcl_dir = exe_dir.join("tcl");
            if tcl_dir.exists() {
                return tcl_dir;
            }
        }
    }

    // Default to ./tcl
    PathBuf::from("./tcl")
}

pub fn http_commands() -> String {
    let tcl_dir = get_tcl_dir();
    let file_path = tcl_dir.join("http.tcl");

    match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to load http.tcl: {}. Using embedded version.", e);
            include_str!("../tcl/http.tcl").to_string()
        }
    }
}

pub fn stocks_commands() -> String {
    let tcl_dir = get_tcl_dir();
    let file_path = tcl_dir.join("stocks.tcl");

    match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to load stocks.tcl: {}. Using embedded version.", e);
            include_str!("../tcl/stocks.tcl").to_string()
        }
    }
}

/// The requester's tag: the SOCKS user name `httpx::proxy_options` uses,
/// so a Tor port with `IsolateSOCKSAuth` gives each requester a circuit
/// of its own. A hash, so it is safe in a user name and says nothing
/// about who it is.
pub fn requester_tag(network: &str, nick: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (network, nick.to_lowercase()).hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The script that sets where the next requests leave from (see
/// `tcl/http.tcl`): the proxy (none: this host's address), whether the
/// requester is the owner (their requests leave directly), and their
/// tag. Run by the host before every evaluation and every event.
pub fn http_context_script(proxy: Option<&str>, owner: bool, network: &str, nick: &str) -> String {
    use crate::tcl_escape::tcl_escape_arg;
    let proxy = proxy.map(str::trim).unwrap_or_default();
    format!(
        "namespace eval ::httpx {{ set proxy {}; set direct {}; set who {} }}",
        tcl_escape_arg(proxy),
        u8::from(owner),
        requester_tag(network, nick),
    )
}
