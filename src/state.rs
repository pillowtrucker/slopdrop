use anyhow::{anyhow, Result};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use tcl::Interpreter;
use tracing::{debug, warn};

/// Represents the state of procs and vars in the interpreter
#[derive(Debug, Clone)]
pub struct InterpreterState {
    pub procs: HashSet<String>,
    pub vars: HashSet<String>,
}

impl InterpreterState {
    /// Capture the current state of the interpreter
    pub fn capture(interp: &Interpreter) -> Result<Self> {
        let procs = Self::get_procs(interp)?;
        let vars = Self::get_vars(interp)?;

        Ok(Self { procs, vars })
    }

    fn get_procs(interp: &Interpreter) -> Result<HashSet<String>> {
        match interp.eval("info procs") {
            Ok(obj) => {
                let procs_str = obj.get_string();
                Ok(procs_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect())
            }
            Err(e) => Err(anyhow!("Failed to get procs: {:?}", e)),
        }
    }

    fn get_vars(interp: &Interpreter) -> Result<HashSet<String>> {
        match interp.eval("info globals") {
            Ok(obj) => {
                let vars_str = obj.get_string();
                Ok(vars_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect())
            }
            Err(e) => Err(anyhow!("Failed to get vars: {:?}", e)),
        }
    }

    /// Find what changed between two states
    pub fn diff(&self, other: &Self) -> StateChanges {
        StateChanges {
            new_procs: other.procs.difference(&self.procs).cloned().collect(),
            deleted_procs: self.procs.difference(&other.procs).cloned().collect(),
            new_vars: other.vars.difference(&self.vars).cloned().collect(),
            deleted_vars: self.vars.difference(&other.vars).cloned().collect(),
        }
    }
}

#[derive(Debug)]
pub struct StateChanges {
    pub new_procs: Vec<String>,
    pub deleted_procs: Vec<String>,
    pub new_vars: Vec<String>,
    pub deleted_vars: Vec<String>,
}

impl StateChanges {
    pub fn has_changes(&self) -> bool {
        !self.new_procs.is_empty()
            || !self.deleted_procs.is_empty()
            || !self.new_vars.is_empty()
            || !self.deleted_vars.is_empty()
    }
}

/// Manages state persistence to disk
pub struct StatePersistence {
    state_path: PathBuf,
}

impl StatePersistence {
    pub fn new(state_path: PathBuf) -> Self {
        Self { state_path }
    }

    /// Save changed procs and vars to disk
    pub fn save_changes(
        &self,
        interp: &Interpreter,
        changes: &StateChanges,
    ) -> Result<()> {
        // Save new/modified procs
        for proc_name in &changes.new_procs {
            if let Err(e) = self.save_proc(interp, proc_name) {
                warn!("Failed to save proc {}: {}", proc_name, e);
            }
        }

        // Save new/modified vars
        for var_name in &changes.new_vars {
            if let Err(e) = self.save_var(interp, var_name) {
                warn!("Failed to save var {}: {}", var_name, e);
            }
        }

        // Delete removed procs
        for proc_name in &changes.deleted_procs {
            if let Err(e) = self.delete_proc(proc_name) {
                warn!("Failed to delete proc {}: {}", proc_name, e);
            }
        }

        // Delete removed vars
        for var_name in &changes.deleted_vars {
            if let Err(e) = self.delete_var(var_name) {
                warn!("Failed to delete var {}: {}", var_name, e);
            }
        }

        Ok(())
    }

    fn save_proc(&self, interp: &Interpreter, proc_name: &str) -> Result<()> {
        // Get proc args and body
        let args_cmd = format!("info args {{{}}}", proc_name);
        let body_cmd = format!("info body {{{}}}", proc_name);

        let args = interp
            .eval(args_cmd.as_str())
            .map_err(|e| anyhow!("Failed to get args for {}: {:?}", proc_name, e))?
            .get_string();

        let body = interp
            .eval(body_cmd.as_str())
            .map_err(|e| anyhow!("Failed to get body for {}: {:?}", proc_name, e))?
            .get_string();

        // Format as {args} {body}
        let content = format!("{{{}}} {{{}}}", args, body);

        // Calculate SHA1 hash
        let hash = Self::sha1_hash(&content);

        // Create procs directory if needed
        let procs_dir = self.state_path.join("procs");
        fs::create_dir_all(&procs_dir)?;

        // Write proc content to file
        let proc_file = procs_dir.join(&hash);
        fs::write(&proc_file, &content)?;

        // Update index
        self.update_proc_index(proc_name, &hash)?;

        debug!("Saved proc {} to {}", proc_name, hash);
        Ok(())
    }

    fn save_var(&self, interp: &Interpreter, var_name: &str) -> Result<()> {
        // Check if it's an array or scalar
        let is_array_cmd = format!("array exists {{{}}}", var_name);
        let is_array = interp
            .eval(is_array_cmd.as_str())
            .map(|obj| obj.get_string() == "1")
            .unwrap_or(false);

        let content = if is_array {
            // Get array contents
            let array_cmd = format!("array get {{{}}}", var_name);
            let array_data = interp
                .eval(array_cmd.as_str())
                .map_err(|e| anyhow!("Failed to get array {}: {:?}", var_name, e))?
                .get_string();
            format!("array {}", array_data)
        } else {
            // Get scalar value
            let value_cmd = format!("set {{{}}}", var_name);
            let value = interp
                .eval(value_cmd.as_str())
                .map_err(|e| anyhow!("Failed to get var {}: {:?}", var_name, e))?
                .get_string();
            format!("scalar {}", value)
        };

        // Calculate SHA1 hash
        let hash = Self::sha1_hash(&content);

        // Create vars directory if needed
        let vars_dir = self.state_path.join("vars");
        fs::create_dir_all(&vars_dir)?;

        // Write var content to file
        let var_file = vars_dir.join(&hash);
        fs::write(&var_file, &content)?;

        // Update index
        self.update_var_index(var_name, &hash)?;

        debug!("Saved var {} to {}", var_name, hash);
        Ok(())
    }

    fn delete_proc(&self, proc_name: &str) -> Result<()> {
        // Remove from index
        let index_path = self.state_path.join("procs/_index");
        if !index_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&index_path)?;
        let new_content: Vec<String> = content
            .lines()
            .filter(|line| !line.starts_with(&format!("{} ", proc_name)))
            .map(|s| s.to_string())
            .collect();

        fs::write(&index_path, new_content.join("\n"))?;
        debug!("Deleted proc {} from index", proc_name);
        Ok(())
    }

    fn delete_var(&self, var_name: &str) -> Result<()> {
        // Remove from index
        let index_path = self.state_path.join("vars/_index");
        if !index_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&index_path)?;
        let new_content: Vec<String> = content
            .lines()
            .filter(|line| !line.starts_with(&format!("{} ", var_name)))
            .map(|s| s.to_string())
            .collect();

        fs::write(&index_path, new_content.join("\n"))?;
        debug!("Deleted var {} from index", var_name);
        Ok(())
    }

    fn update_proc_index(&self, proc_name: &str, hash: &str) -> Result<()> {
        let index_path = self.state_path.join("procs/_index");

        // Read existing index
        let mut entries: HashMap<String, String> = HashMap::new();
        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    entries.insert(parts[0].to_string(), parts[1].to_string());
                }
            }
        }

        // Update entry
        entries.insert(proc_name.to_string(), hash.to_string());

        // Write back sorted
        let mut lines: Vec<String> = entries
            .iter()
            .map(|(name, hash)| format!("{} {}", name, hash))
            .collect();
        lines.sort();

        fs::write(&index_path, lines.join("\n"))?;
        Ok(())
    }

    fn update_var_index(&self, var_name: &str, hash: &str) -> Result<()> {
        let index_path = self.state_path.join("vars/_index");

        // Read existing index
        let mut entries: HashMap<String, String> = HashMap::new();
        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    entries.insert(parts[0].to_string(), parts[1].to_string());
                }
            }
        }

        // Update entry
        entries.insert(var_name.to_string(), hash.to_string());

        // Write back sorted
        let mut lines: Vec<String> = entries
            .iter()
            .map(|(name, hash)| format!("{} {}", name, hash))
            .collect();
        lines.sort();

        fs::write(&index_path, lines.join("\n"))?;
        Ok(())
    }

    fn sha1_hash(content: &str) -> String {
        let mut hasher = Sha1::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
