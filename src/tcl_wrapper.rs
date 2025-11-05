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
    /// Create a new safe TCL interpreter
    pub fn new(timeout_ms: u64, state_path: &Path) -> Result<Self> {
        // Create a new TCL interpreter (safe mode will be applied next)
        let interpreter = Interpreter::new().map_err(|e| anyhow!("Failed to create TCL interpreter: {:?}", e))?;

        // Make the interpreter safe
        Self::setup_safe_interp(&interpreter)?;

        // Load state if it exists
        if state_path.exists() {
            debug!("Loading TCL state from {:?}", state_path);
            // TODO: Implement state loading
        }

        Ok(Self {
            interpreter,
            _timeout_ms: timeout_ms,
        })
    }

    /// Configure the interpreter to be safe
    fn setup_safe_interp(interp: &Interpreter) -> Result<()> {
        // Hide dangerous commands that could break out of sandbox
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
            "socket",
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

    /// Evaluate TCL code with timeout protection
    pub fn eval(&self, code: &str) -> Result<String> {
        // TODO: Implement proper timeout mechanism
        // The original used SIGALRM, we might need a different approach with threads

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
