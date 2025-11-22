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

pub fn stock_wrappers() -> String {
    let tcl_dir = get_tcl_dir();
    let file_path = tcl_dir.join("stock_wrappers.tcl");

    match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to load stock_wrappers.tcl: {}. Using embedded version.", e);
            include_str!("../tcl/stock_wrappers.tcl").to_string()
        }
    }
}
