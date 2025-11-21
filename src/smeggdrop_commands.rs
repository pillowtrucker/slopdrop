/// Smeggdrop TCL commands that need to be injected into the interpreter
/// These replicate functionality from the original bot
///
/// TCL scripts are stored in tcl/ directory and loaded at runtime for hot-reloading

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

/// Load TCL file with fallback to embedded version
fn load_tcl_file(filename: &str, embedded: &'static str) -> String {
    let tcl_dir = get_tcl_dir();
    let file_path = tcl_dir.join(filename);

    match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to load {}: {}. Using embedded version.", filename, e);
            embedded.to_string()
        }
    }
}

/// Returns the cache commands TCL code
pub fn cache_commands() -> String {
    load_tcl_file("cache.tcl", include_str!("../tcl/cache.tcl"))
}

/// Returns utility commands TCL code
pub fn utility_commands() -> String {
    load_tcl_file("utils.tcl", include_str!("../tcl/utils.tcl"))
}

/// Returns the encoding commands TCL code
pub fn encoding_commands() -> String {
    load_tcl_file("encoding.tcl", include_str!("../tcl/encoding.tcl"))
}

/// Returns SHA1 hashing command
pub fn sha1_command() -> String {
    load_tcl_file("sha1.tcl", include_str!("../tcl/sha1.tcl"))
}

/// Returns ImageMagick placeholder commands
pub fn magick_commands() -> String {
    load_tcl_file("magick.tcl", include_str!("../tcl/magick.tcl"))
}

/// Returns TIMTOM bot commands (ported from mIRC)
pub fn timtom_commands() -> String {
    load_tcl_file("timtom.tcl", include_str!("../tcl/timtom.tcl"))
}

/// Returns trigger/event binding system
pub fn trigger_commands() -> String {
    load_tcl_file("triggers.tcl", include_str!("../tcl/triggers.tcl"))
}

/// Returns general timer infrastructure
pub fn timer_commands() -> String {
    load_tcl_file("timers.tcl", include_str!("../tcl/timers.tcl"))
}

/// Returns procedure tracking wrapper for efficient change detection
pub fn proc_tracking() -> String {
    load_tcl_file("proc_tracking.tcl", include_str!("../tcl/proc_tracking.tcl"))
}

/// Initialize all smeggdrop commands in the interpreter
/// NOTE: Currently unused - we call individual command loaders in tcl_wrapper.rs
/// to control loading order (some must load before making interpreter safe).
/// Kept for reference.
#[allow(dead_code)]
pub fn inject_commands(interp: &tcl::Interpreter) -> anyhow::Result<()> {
    use tracing::debug;

    debug!("Injecting smeggdrop commands");

    // Inject cache commands
    interp.eval(cache_commands().as_str())
        .map_err(|e| anyhow::anyhow!("Failed to inject cache commands: {:?}", e))?;

    // Inject utility commands
    interp.eval(utility_commands().as_str())
        .map_err(|e| anyhow::anyhow!("Failed to inject utility commands: {:?}", e))?;

    // Inject encoding commands
    interp.eval(encoding_commands().as_str())
        .map_err(|e| anyhow::anyhow!("Failed to inject encoding commands: {:?}", e))?;

    // Inject SHA1 command (placeholder)
    interp.eval(sha1_command().as_str())
        .map_err(|e| anyhow::anyhow!("Failed to inject SHA1 command: {:?}", e))?;

    debug!("Smeggdrop commands injected successfully");

    Ok(())
}
