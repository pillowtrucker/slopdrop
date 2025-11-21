use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use tcl::Interpreter;
use tracing::debug;

/// Sanitize error messages to prevent information disclosure
/// Removes filesystem paths and other sensitive information
fn sanitize_error_message(error_msg: &str) -> String {
    // Remove absolute paths (Unix and Windows style)
    // Match patterns like /home/user/..., /var/..., C:\Users\..., etc.
    let re_unix_path = Regex::new(r"/[a-zA-Z0-9_/.-]+").unwrap();
    let re_win_path = Regex::new(r"[A-Za-z]:\\[a-zA-Z0-9_\\.-]+").unwrap();

    let mut sanitized = error_msg.to_string();

    // Replace Unix paths with generic placeholder
    sanitized = re_unix_path.replace_all(&sanitized, "[PATH]").to_string();

    // Replace Windows paths with generic placeholder
    sanitized = re_win_path.replace_all(&sanitized, "[PATH]").to_string();

    // Remove any remaining file:// URLs
    let re_file_url = Regex::new(r"file://[^\s]+").unwrap();
    sanitized = re_file_url.replace_all(&sanitized, "[FILE-URL]").to_string();

    sanitized
}

/// Wrapper around a TCL interpreter with safety features
/// Note: This is not Send/Sync due to TCL interpreter limitations
/// It should be created and used within a single thread
pub struct SafeTclInterp {
    interpreter: Interpreter,
    _timeout_ms: u64,
}

impl SafeTclInterp {
    /// Get a reference to the underlying interpreter
    pub fn interpreter(&self) -> &Interpreter {
        &self.interpreter
    }
}

impl SafeTclInterp {
    /// Create a new safe TCL interpreter
    pub fn new(timeout_ms: u64, state_path: &Path, state_repo: Option<String>, ssh_key: Option<PathBuf>, max_recursion_depth: u32) -> Result<Self> {
        // Create a new TCL interpreter (safe mode will be applied next)
        let interpreter = Interpreter::new().map_err(|e| anyhow!("Failed to create TCL interpreter: {:?}", e))?;

        // Set recursion limit (0 = no limit, use TCL default)
        if max_recursion_depth > 0 {
            let recursion_cmd = format!("interp recursionlimit {{}} {}", max_recursion_depth);
            interpreter.eval(recursion_cmd.as_str())
                .map_err(|e| anyhow!("Failed to set recursion limit: {:?}", e))?;
            debug!("Set TCL recursion limit to {}", max_recursion_depth);
        }

        // Add tcllib path to auto_path for package loading (sha1, etc.)
        let _ = interpreter.eval("lappend auto_path /usr/share/tcltk");

        // Inject HTTP commands BEFORE making interpreter safe
        // (package require needs unrestricted access to TCL package system)
        if let Err(e) = interpreter.eval(crate::http_tcl_commands::http_commands().as_str()) {
            debug!("HTTP commands not available (http package missing): {:?}", e);
            debug!("Bot will work but HTTP commands will not be available");
        }

        // Inject SHA1 command BEFORE making interpreter safe
        // (package require sha1 needs unrestricted access to TCL package system)
        if let Err(e) = interpreter.eval(crate::smeggdrop_commands::sha1_command().as_str()) {
            debug!("SHA1 command not available (tcllib sha1 package missing): {:?}", e);
            debug!("Bot will work but SHA1 command will not be available");
        }

        // Force-load the clock package BEFORE making interpreter safe
        // TCL loads clock.tcl lazily via [source [file join $tcl_library clock.tcl]]
        // which fails after we block source/file. Loading it now ensures it's available.
        // We need to trigger ALL clock subcommands that do lazy loading:
        // - clock seconds: basic time
        // - clock format: loads formatting code
        // - clock scan: loads parsing code
        // - clock clicks: usually built-in
        let clock_init = r#"
            clock seconds
            clock format [clock seconds]
            clock scan "now"
            clock clicks
        "#;
        if let Err(e) = interpreter.eval(clock_init) {
            debug!("Clock command initialization failed: {:?}", e);
        }

        // Load TclCurl package for HTTP/HTTPS support BEFORE making interpreter safe
        // TclCurl handles HTTPS natively through libcurl
        let curl_init = r#"
            package require TclCurl
        "#;
        if let Err(e) = interpreter.eval(curl_init) {
            debug!("TclCurl package not available (HTTP/HTTPS will not work): {:?}", e);
        }

        // Make the interpreter safe (AFTER loading packages)
        Self::setup_safe_interp(&interpreter)?;

        // Load proc tracking wrapper FIRST to intercept all proc definitions
        interpreter.eval(crate::smeggdrop_commands::proc_tracking().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject proc tracking: {:?}", e))?;

        // Inject other smeggdrop commands (cache, utils, encoding)
        // These don't require package loading so they can be loaded after making safe
        interpreter.eval(crate::smeggdrop_commands::cache_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject cache commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::utility_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject utility commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::encoding_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject encoding commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::magick_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject magick commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::timer_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject timer commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::trigger_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject trigger commands: {:?}", e))?;
        interpreter.eval(crate::smeggdrop_commands::timtom_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to inject timtom commands: {:?}", e))?;

        // Stock commands are handled natively in Rust (see stock_commands.rs and tcl_thread.rs)
        // No TCL injection needed - commands are intercepted before TCL evaluation

        // Ensure state directory exists and git repo is initialized
        // If state_repo is set and state doesn't exist, clone from remote
        // Otherwise create empty repo if needed
        if !state_path.exists() {
            debug!("State path doesn't exist, initializing: {:?}", state_path);
            let persistence = crate::state::StatePersistence::with_repo(
                state_path.to_path_buf(),
                state_repo.clone(),
                ssh_key.clone(),
            );
            // This will clone from remote or create directory structure
            if let Err(e) = persistence.ensure_initialized() {
                debug!("Failed to initialize state directory: {}. Will be created on first save.", e);
            }
        }

        // Load state if it exists
        if state_path.exists() {
            debug!("Loading TCL state from {:?}", state_path);
            Self::load_state(&interpreter, state_path)?;

            // Clear the modified procs/vars list after loading state
            // State loading triggers the proc wrapper, but these are initialization procs
            // not actual modifications, so we clear the tracking lists
            debug!("Clearing modified tracking after state load");
            let _ = interpreter.eval("set ::slopdrop_modified_procs [list]");
            let _ = interpreter.eval("set ::slopdrop_modified_vars [list]");
        }

        Ok(Self {
            interpreter,
            _timeout_ms: timeout_ms,
        })
    }

    /// Configure the interpreter to be safe
    fn setup_safe_interp(interp: &Interpreter) -> Result<()> {
        // Hide dangerous commands that could break out of sandbox
        // Note: socket is allowed for http package (protected by timeout and rate limiting)
        // Note: namespace is allowed for cache commands to work
        // Note: apply is allowed - needed for functional programming and stolen-treasure.tcl
        let dangerous_commands = vec![
            "interp",
            // "namespace",  // Allowed for cache/utils commands
            // "trace",  // Allowed - needed for variable modification tracking
            // "vwait",  // Allowed - needed by http package for event loop
            // "apply",  // Allowed - needed for lambdas and stolen-treasure.tcl
            "yield",
            "exec",
            "open",
            "file",
            // "socket",  // Allowed for http package
            "source",
            "load",
            "cd",
            "pwd",
            "glob",
            "exit",
        ];

        for cmd in dangerous_commands {
            // Try to rename the command to make it unavailable
            let rename_cmd = format!("catch {{rename {} {{}}}}", cmd);
            let _ = interp.eval(rename_cmd.as_str());
        }

        // Note: Proc tracking is handled via state diff mechanism in tcl_thread.rs
        // We capture full state before/after each eval and detect changes

        // Define context commands that return cached variables
        // These are set once and read from global variables updated before each eval
        let _ = interp.eval("proc nick {} {return $::nick}");
        let _ = interp.eval("proc channel {} {return $::channel}");
        let _ = interp.eval("proc mask {} {return $::mask}");

        debug!("Safe TCL interpreter configured");
        Ok(())
    }

    /// Load state from the state directory
    fn load_state(interp: &Interpreter, state_path: &Path) -> Result<()> {
        // 1. Load stolen-treasure.tcl (base library)
        let stolen_treasure = state_path.join("stolen-treasure.tcl");
        if stolen_treasure.exists() {
            debug!("Loading stolen-treasure.tcl");
            let content = std::fs::read_to_string(&stolen_treasure)?;
            interp.eval(content.as_str()).map_err(|e| anyhow!("Failed to load stolen-treasure.tcl: {:?}", e))?;
        }

        // 2. Load restore_missing_vars.tcl (variable restoration script)
        let restore_vars = state_path.join("restore_missing_vars.tcl");
        if restore_vars.exists() {
            debug!("Loading restore_missing_vars.tcl");
            let content = std::fs::read_to_string(&restore_vars)?;
            interp.eval(content.as_str()).map_err(|e| anyhow!("Failed to load restore_missing_vars.tcl: {:?}", e))?;
        }

        // 3. Set up english_words as a small default list
        // Note: Loading the full 370K+ word dictionary causes timeouts
        // Using a smaller embedded list instead
        debug!("Setting up english_words with default word list");
        let default_words = "set english_words {the be to of and a in that have I it for not on with he as you do at this but his by from they we say her she or an will my one all would there their what so up out if about who get which go me when make can like time no just him know take people into year your good some could them see other than then now look only come its over think also back after use two how our work first well way even new want because any these give day most us}";
        interp.eval(default_words).map_err(|e| anyhow!("Failed to set up english_words: {:?}", e))?;

        // 4. Load procs from procs/_index
        let procs_index = state_path.join("procs/_index");
        if procs_index.exists() {
            debug!("Loading procs from state");
            let index_content = std::fs::read_to_string(&procs_index)?;
            for line in index_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let proc_name = parts[0];
                    let file_hash = parts[1];
                    let proc_file = state_path.join("procs").join(file_hash);

                    if proc_file.exists() {
                        let proc_content = std::fs::read_to_string(&proc_file)?;
                        // proc_content is: {args} {body}
                        let proc_def = format!("proc {{{}}} {}", proc_name, proc_content);
                        if let Err(e) = interp.eval(proc_def.as_str()) {
                            debug!("Warning: Failed to load proc {}: {:?}", proc_name, e);
                        }
                    }
                }
            }
        }

        // 5. Load vars from vars/_index
        let vars_index = state_path.join("vars/_index");
        if vars_index.exists() {
            debug!("Loading vars from state");
            let index_content = std::fs::read_to_string(&vars_index)?;
            for line in index_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let var_name = parts[0];
                    let file_hash = parts[1];
                    let var_file = state_path.join("vars").join(file_hash);

                    if var_file.exists() {
                        let var_content = std::fs::read_to_string(&var_file)?;
                        // var_content is either: "scalar {value}" or "array {key value key value}"
                        // Values are TCL-quoted (with braces) directly from the historical format
                        if var_content.starts_with("scalar ") {
                            let value = var_content.strip_prefix("scalar ").unwrap_or("");
                            // Value is TCL-quoted, use it directly
                            let set_cmd = format!("set {{{}}} {}", var_name, value);
                            if let Err(e) = interp.eval(set_cmd.as_str()) {
                                debug!("Warning: Failed to load var {}: {:?}", var_name, e);
                            }
                        } else if var_content.starts_with("array ") {
                            let array_data = var_content.strip_prefix("array ").unwrap_or("");
                            // Array data is TCL-quoted, use it directly
                            let array_cmd = format!("array set {{{}}} {}", var_name, array_data);
                            if let Err(e) = interp.eval(array_cmd.as_str()) {
                                debug!("Warning: Failed to load array {}: {:?}", var_name, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Evaluate TCL code
    ///
    /// Note: Timeout is handled at the thread level (see tcl_thread.rs)
    /// This method is called from within the TCL worker thread
    pub fn eval(&self, code: &str) -> Result<String> {
        match self.interpreter.eval(code) {
            Ok(obj) => {
                let result = obj.get_string();
                Ok(result)
            }
            Err(e) => {
                // Get error info
                let error_info = self.interpreter
                    .eval("set errorInfo")
                    .ok()
                    .map(|obj| obj.get_string())
                    .unwrap_or_else(|| format!("{:?}", e));

                // Sanitize error message to prevent path disclosure
                let sanitized = sanitize_error_message(&error_info);

                Err(anyhow!("TCL Error: {}", sanitized))
            }
        }
    }

    /// Evaluate code with user context (for pub:tcl:perform emulation)
    pub fn eval_with_context(
        &self,
        code: &str,
        nick: &str,
        mask: &str,
        channel: &str,
    ) -> Result<String> {
        // Set context variables that the nick/channel/mask procs will read
        // The procs are defined once in setup_safe_interp
        let _ = self.interpreter.eval(format!("set ::nick {{{}}}", nick).as_str());
        let _ = self.interpreter.eval(format!("set ::channel {{{}}}", channel).as_str());
        let _ = self.interpreter.eval(format!("set ::mask {{{}}}", mask).as_str());

        self.eval(code)
    }

    /// Get a list of user-defined procs
    /// NOTE: Currently unused - state diff in tcl_thread.rs handles proc tracking
    #[allow(dead_code)]
    pub fn get_procs(&self) -> Result<Vec<String>> {
        match self.interpreter.eval("info procs") {
            Ok(obj) => {
                let procs_str = obj.get_string();
                Ok(procs_str.split_whitespace().map(|s| s.to_string()).collect())
            }
            Err(e) => Err(anyhow!("Failed to get procs: {:?}", e)),
        }
    }

    /// Get a list of global variables
    /// NOTE: Currently unused - state diff in tcl_thread.rs handles var tracking
    #[allow(dead_code)]
    pub fn get_vars(&self) -> Result<Vec<String>> {
        match self.interpreter.eval("info globals") {
            Ok(obj) => {
                let vars_str = obj.get_string();
                Ok(vars_str.split_whitespace().map(|s| s.to_string()).collect())
            }
            Err(e) => Err(anyhow!("Failed to get vars: {:?}", e)),
        }
    }

    /// Save interpreter state to disk
    ///
    /// NOTE: Legacy method, currently unused. State persistence is now handled
    /// automatically in tcl_thread.rs using StatePersistence
    #[allow(dead_code)]
    pub fn save_state(&self, _state_path: &Path) -> Result<()> {
        debug!("State saving handled by StatePersistence in tcl_thread.rs");
        Ok(())
    }

    /// Reload all TCL modules from disk
    ///
    /// This reloads all the TCL module files (http, cache, utils, etc.) from the tcl/ directory.
    /// This is useful for hot-reloading changes without restarting the bot.
    ///
    /// Note: This does NOT reload user-defined procs/vars from state - those are persistent.
    /// This only reloads the built-in TCL modules.
    pub fn reload_modules(&self) -> Result<()> {
        debug!("Reloading TCL modules from disk");

        // Reload HTTP commands (must be done before safety was applied, but interpreter is already safe)
        // We can still eval new code in a safe interpreter
        if let Err(e) = self.interpreter.eval(crate::http_tcl_commands::http_commands().as_str()) {
            debug!("Failed to reload HTTP commands: {:?}", e);
        }

        // Reload SHA1 command
        if let Err(e) = self.interpreter.eval(crate::smeggdrop_commands::sha1_command().as_str()) {
            debug!("Failed to reload SHA1 command: {:?}", e);
        }

        // Reload proc tracking wrapper FIRST to intercept all proc definitions
        self.interpreter.eval(crate::smeggdrop_commands::proc_tracking().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload proc tracking: {:?}", e))?;

        // Reload other smeggdrop commands
        self.interpreter.eval(crate::smeggdrop_commands::cache_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload cache commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::utility_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload utility commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::encoding_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload encoding commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::magick_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload magick commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::timer_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload timer commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::trigger_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload trigger commands: {:?}", e))?;
        self.interpreter.eval(crate::smeggdrop_commands::timtom_commands().as_str())
            .map_err(|e| anyhow::anyhow!("Failed to reload timtom commands: {:?}", e))?;

        debug!("TCL modules reloaded successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_basic_eval() {
        let state_path = PathBuf::from("/tmp/tcl_test_state");
        let interp = SafeTclInterp::new(30000, &state_path, None, None, 1000).unwrap();

        let result = interp.eval("expr {1 + 1}").unwrap();
        assert_eq!(result.trim(), "2");
    }

    #[test]
    fn test_dangerous_commands_blocked() {
        let state_path = PathBuf::from("/tmp/tcl_test_state");
        let interp = SafeTclInterp::new(30000, &state_path, None, None, 1000).unwrap();

        // These should fail or be unavailable
        let result = interp.eval("exec ls");
        assert!(result.is_err());
    }

    #[test]
    fn test_proc_creation() {
        let state_path = PathBuf::from("/tmp/tcl_test_state");
        let interp = SafeTclInterp::new(30000, &state_path, None, None, 1000).unwrap();

        interp.eval("proc hello {} { return \"world\" }").unwrap();
        let result = interp.eval("hello").unwrap();
        assert_eq!(result.trim(), "world");
    }
}
