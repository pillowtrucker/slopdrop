use anyhow::{anyhow, Result};
use std::path::Path;
use tcl::Interpreter;
use tracing::debug;

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
    pub fn new(timeout_ms: u64, state_path: &Path) -> Result<Self> {
        // Create a new TCL interpreter (safe mode will be applied next)
        let interpreter = Interpreter::new().map_err(|e| anyhow!("Failed to create TCL interpreter: {:?}", e))?;

        // Make the interpreter safe
        Self::setup_safe_interp(&interpreter)?;

        // Inject smeggdrop commands
        crate::smeggdrop_commands::inject_commands(&interpreter)?;

        // Inject HTTP commands
        interpreter.eval(crate::http_tcl_commands::http_commands())
            .map_err(|e| anyhow!("Failed to inject HTTP commands: {:?}", e))?;

        // Load state if it exists
        if state_path.exists() {
            debug!("Loading TCL state from {:?}", state_path);
            Self::load_state(&interpreter, state_path)?;
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
        let dangerous_commands = vec![
            "interp",
            "namespace",
            "trace",
            "vwait",
            "apply",
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

        // Set up proc command wrapper to track user-defined procs
        // TODO: Add proc tracking for state persistence

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

        // 2. Load english_words.txt as a TCL variable
        let english_words = state_path.join("english_words.txt");
        if english_words.exists() {
            debug!("Loading english_words.txt");
            let content = std::fs::read_to_string(&english_words)?;
            let lines: Vec<&str> = content.lines().collect();
            // Create a TCL list
            let tcl_list = format!("set english_words [list {}]", lines.join(" "));
            interp.eval(tcl_list.as_str()).map_err(|e| anyhow!("Failed to load english_words: {:?}", e))?;
        }

        // 3. Load procs from procs/_index
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

        // 4. Load vars from vars/_index
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
                        // var_content is either: "scalar value" or "array {key value key value}"
                        if var_content.starts_with("scalar ") {
                            let value = var_content.strip_prefix("scalar ").unwrap_or("");
                            let set_cmd = format!("set {{{}}} {{{}}}", var_name, value);
                            if let Err(e) = interp.eval(set_cmd.as_str()) {
                                debug!("Warning: Failed to load var {}: {:?}", var_name, e);
                            }
                        } else if var_content.starts_with("array ") {
                            let array_data = var_content.strip_prefix("array ").unwrap_or("");
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

    /// Evaluate TCL code with timeout protection
    ///
    /// WARNING: Timeout is not yet implemented. Infinite loops will hang!
    /// TODO: Implement proper timeout mechanism using one of:
    /// - TCL's interp limit command (time/command limits)
    /// - Separate process with watchdog
    /// - Signal-based interruption (SIGALRM on Unix)
    /// The original used SIGALRM which is not portable to async Rust
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

                Err(anyhow!("TCL Error: {}", error_info))
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
        // Set context variables using eval
        let _ = self.interpreter.eval(format!("set ::nick {{{}}}", nick).as_str());
        let _ = self.interpreter.eval(format!("set ::channel {{{}}}", channel).as_str());
        let _ = self.interpreter.eval(format!("set ::mask {{{}}}", mask).as_str());

        self.eval(code)
    }

    /// Get a list of user-defined procs
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
    pub fn save_state(&self, _state_path: &Path) -> Result<()> {
        // TODO: Implement git-based state persistence like the original
        debug!("State saving not yet implemented");
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
        let interp = SafeTclInterp::new(30000, &state_path).unwrap();

        let result = interp.eval("expr {1 + 1}").unwrap();
        assert_eq!(result.trim(), "2");
    }

    #[test]
    fn test_dangerous_commands_blocked() {
        let state_path = PathBuf::from("/tmp/tcl_test_state");
        let interp = SafeTclInterp::new(30000, &state_path).unwrap();

        // These should fail or be unavailable
        let result = interp.eval("exec ls");
        assert!(result.is_err());
    }

    #[test]
    fn test_proc_creation() {
        let state_path = PathBuf::from("/tmp/tcl_test_state");
        let interp = SafeTclInterp::new(30000, &state_path).unwrap();

        interp.eval("proc hello {} { return \"world\" }").unwrap();
        let result = interp.eval("hello").unwrap();
        assert_eq!(result.trim(), "world");
    }
}
