use anyhow::{anyhow, Result};
use git2::{Repository, Signature, IndexAddOption, Cred, RemoteCallbacks, PushOptions, FetchOptions, build::RepoBuilder};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use tcl::Interpreter;
use tracing::{debug, info, warn};

/// Information about a git commit
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    pub commit_id: String,
    pub author: String,
    pub message: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    /// Summary of which procs/vars were created/updated/removed
    pub changes_summary: String,
}

/// IRC user information for git commits
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub nick: String,
    pub host: String,
}

impl UserInfo {
    pub fn new(nick: String, host: String) -> Self {
        Self { nick, host }
    }

    /// Generate a git author signature from IRC user info
    pub fn to_signature(&self) -> Result<Signature<'static>> {
        let email = format!("{}@{}", self.nick, self.host);
        Signature::now(&self.nick, &email)
            .map_err(|e| anyhow!("Failed to create signature: {}", e))
    }
}

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
        // Internal context variables that should not be tracked as state changes
        // These are set by eval_with_context for each command, or are system arrays
        let internal_vars: HashSet<String> = [
            "nick", "channel", "mask",  // Context variables set per-eval
            "slopdrop_channel_members", // Channel member lists synced before each eval
            "slopdrop_log_lines",       // Message log array
            "nick_channel",             // HTTP rate limiting context
            "_english_words_cache",     // Lazy-loaded english words cache
        ]
            .iter()
            .map(|s| s.to_string())
            .collect();

        StateChanges {
            new_procs: other.procs.difference(&self.procs).cloned().collect(),
            deleted_procs: self.procs.difference(&other.procs).cloned().collect(),
            new_vars: other.vars.difference(&self.vars)
                .filter(|v| !internal_vars.contains(*v))
                .cloned()
                .collect(),
            deleted_vars: self.vars.difference(&other.vars)
                .filter(|v| !internal_vars.contains(*v))
                .cloned()
                .collect(),
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

    /// Generate a human-readable summary of changes
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if !self.new_procs.is_empty() {
            parts.push(format!("+proc: {}", self.new_procs.join(", ")));
        }
        if !self.deleted_procs.is_empty() {
            parts.push(format!("-proc: {}", self.deleted_procs.join(", ")));
        }
        if !self.new_vars.is_empty() {
            parts.push(format!("+var: {}", self.new_vars.join(", ")));
        }
        if !self.deleted_vars.is_empty() {
            parts.push(format!("-var: {}", self.deleted_vars.join(", ")));
        }

        if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

/// Manages state persistence to disk
pub struct StatePersistence {
    state_path: PathBuf,
    state_repo: Option<String>,
    ssh_key: Option<PathBuf>,
}

impl StatePersistence {
    /// Create a new StatePersistence without remote repository support
    /// NOTE: Prefer using with_repo() to support remote state cloning
    #[allow(dead_code)]
    pub fn new(state_path: PathBuf) -> Self {
        Self {
            state_path,
            state_repo: None,
            ssh_key: None,
        }
    }

    pub fn with_repo(state_path: PathBuf, state_repo: Option<String>, ssh_key: Option<PathBuf>) -> Self {
        Self {
            state_path,
            state_repo,
            ssh_key,
        }
    }

    /// Ensure state directory and git repository are initialized
    /// If state_repo is set and state_path doesn't exist, clones from remote
    /// Otherwise creates directory structure and initializes git repo if needed
    /// This is called on bot startup to ensure state is ready
    pub fn ensure_initialized(&self) -> Result<()> {
        // If state doesn't exist and we have a remote URL, clone it
        if !self.state_path.exists() {
            if let Some(ref repo_url) = self.state_repo {
                info!("Cloning state from remote repository: {}", repo_url);
                return self.clone_from_remote(repo_url);
            }
        }

        // Otherwise initialize normally (create empty repo if needed)
        self.init_git_repo_if_needed()?;
        Ok(())
    }

    /// Clone state repository from remote URL
    fn clone_from_remote(&self, url: &str) -> Result<()> {
        info!("Cloning state repository from: {}", url);

        // Set up credentials callback for SSH
        let mut callbacks = RemoteCallbacks::new();
        let ssh_key = self.ssh_key.clone();

        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            let username = username_from_url.unwrap_or("git");

            // Try SSH key if configured
            if let Some(ref key_path) = ssh_key {
                debug!("Using SSH key for clone: {:?}", key_path);
                return Cred::ssh_key(username, None, key_path, None);
            }

            // Fall back to SSH agent
            debug!("Using SSH agent for clone authentication");
            Cred::ssh_key_from_agent(username)
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = RepoBuilder::new();
        builder.fetch_options(fetch_options);

        builder.clone(url, &self.state_path)
            .map_err(|e| anyhow!("Failed to clone state repository from {}: {}", url, e))?;

        info!("Successfully cloned state repository to {:?}", self.state_path);
        Ok(())
    }

    /// Save changed procs and vars to disk and commit to git
    pub fn save_changes(
        &self,
        interp: &Interpreter,
        changes: &StateChanges,
        user_info: &UserInfo,
        eval_code: &str,
    ) -> Result<Option<CommitInfo>> {
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

        // Commit changes to git and return commit info
        match self.git_commit(changes, user_info, eval_code) {
            Ok(commit_info) => {
                // Auto-push to remote if configured
                if let Err(e) = self.push_to_remote() {
                    warn!("Failed to push to remote: {}", e);
                }

                // Run git gc periodically (every 100 commits) to prevent repo bloat
                if let Err(e) = self.maybe_run_git_gc() {
                    warn!("Failed to run git gc: {}", e);
                }

                Ok(Some(commit_info))
            }
            Err(e) => {
                warn!("Failed to commit to git: {}", e);
                Ok(None)
            }
        }
    }

    /// Push changes to remote repository if configured
    /// Supports both HTTPS and SSH (with key or agent)
    pub fn push_to_remote(&self) -> Result<()> {
        // Only push if we have a remote URL configured
        if self.state_repo.is_none() {
            debug!("No remote repository configured, skipping push");
            return Ok(());
        }

        let repo = Repository::open(&self.state_path)
            .map_err(|e| anyhow!("Failed to open git repository: {}", e))?;

        // Find the origin remote or create it
        let remote_name = "origin";
        let remote_url = self.state_repo.as_ref().unwrap();

        // Try to find existing remote
        let mut remote = match repo.find_remote(remote_name) {
            Ok(remote) => remote,
            Err(_) => {
                // Create remote if it doesn't exist
                debug!("Creating remote 'origin' with URL: {}", remote_url);
                repo.remote(remote_name, remote_url)?
            }
        };

        // Set up credentials callback for SSH
        let mut callbacks = RemoteCallbacks::new();
        let ssh_key = self.ssh_key.clone();

        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            let username = username_from_url.unwrap_or("git");

            // Try SSH key if configured
            if let Some(ref key_path) = ssh_key {
                debug!("Using SSH key: {:?}", key_path);
                return Cred::ssh_key(username, None, key_path, None);
            }

            // Fall back to SSH agent
            debug!("Using SSH agent for authentication");
            Cred::ssh_key_from_agent(username)
        });

        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Push to remote
        info!("Pushing to remote repository: {}", remote_url);

        // Try pushing to main first
        if let Err(e) = remote.push(
            &["refs/heads/main:refs/heads/main"],
            Some(&mut push_options)
        ) {
            // If main doesn't exist, try master
            debug!("Push to main failed ({}), trying master", e);

            // Need fresh callbacks for retry
            let mut callbacks2 = RemoteCallbacks::new();
            let ssh_key2 = self.ssh_key.clone();
            callbacks2.credentials(move |_url, username_from_url, _allowed_types| {
                let username = username_from_url.unwrap_or("git");
                if let Some(ref key_path) = ssh_key2 {
                    return Cred::ssh_key(username, None, key_path, None);
                }
                Cred::ssh_key_from_agent(username)
            });

            let mut push_options2 = PushOptions::new();
            push_options2.remote_callbacks(callbacks2);

            remote.push(
                &["refs/heads/master:refs/heads/master"],
                Some(&mut push_options2)
            ).map_err(|e2| anyhow!("Failed to push to main: {} and master: {}", e, e2))?;
            info!("Successfully pushed to master");
        } else {
            info!("Successfully pushed to main");
        }

        Ok(())
    }

    /// Initialize git repository if it doesn't exist
    fn init_git_repo_if_needed(&self) -> Result<()> {
        // Try to open existing repo first
        if Repository::open(&self.state_path).is_ok() {
            return Ok(());
        }

        // Repository doesn't exist, create it
        debug!("Initializing git repository at {:?}", self.state_path);

        // Create state directory if it doesn't exist
        std::fs::create_dir_all(&self.state_path)?;
        std::fs::create_dir_all(self.state_path.join("procs"))?;
        std::fs::create_dir_all(self.state_path.join("vars"))?;

        // Initialize git repo
        let repo = Repository::init(&self.state_path)?;

        // Create initial empty index files
        std::fs::write(self.state_path.join("procs/_index"), "")?;
        std::fs::write(self.state_path.join("vars/_index"), "")?;

        // Create initial commit
        let mut index = repo.index()?;
        index.add_path(std::path::Path::new("procs/_index"))?;
        index.add_path(std::path::Path::new("vars/_index"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let sig = Signature::now("slopdrop", "bot@localhost")?;
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )?;

        info!("Git repository initialized at {:?}", self.state_path);
        Ok(())
    }

    /// Create a git commit with the changes and return commit information
    fn git_commit(
        &self,
        changes: &StateChanges,
        user_info: &UserInfo,
        eval_code: &str,
    ) -> Result<CommitInfo> {
        self.init_git_repo_if_needed()?;
        let repo = Repository::open(&self.state_path)
            .map_err(|e| anyhow!("Failed to open git repository: {}", e))?;

        // Get parent commit for diff stats
        let parent_commit = repo.head()?.peel_to_commit()?;
        let parent_tree = parent_commit.tree()?;

        // Add all changed files to the index
        let mut index = repo.index()
            .map_err(|e| anyhow!("Failed to get git index: {}", e))?;

        // Add procs/_index and vars/_index
        index.add_path(std::path::Path::new("procs/_index"))?;
        index.add_path(std::path::Path::new("vars/_index"))?;

        // Add all new proc files
        if !changes.new_procs.is_empty() {
            index.add_all(["procs/*"].iter(), IndexAddOption::DEFAULT, None)?;
        }

        // Add all new var files
        if !changes.new_vars.is_empty() {
            index.add_all(["vars/*"].iter(), IndexAddOption::DEFAULT, None)?;
        }

        index.write()?;

        // Create commit message
        let commit_msg = Self::format_commit_message(changes, eval_code);

        // Get the tree
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        // Create signature from user info
        let signature = user_info.to_signature()?;

        // Create the commit
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &commit_msg,
            &tree,
            &[&parent_commit],
        )?;

        // Calculate diff stats
        let new_commit = repo.find_commit(commit_id)?;
        let new_tree = new_commit.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&new_tree), None)?;
        let stats = diff.stats()?;

        let commit_info = CommitInfo {
            commit_id: commit_id.to_string(),
            author: user_info.nick.clone(),
            message: commit_msg,
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
            changes_summary: changes.summary(),
        };

        info!(
            "Created git commit {} by {} ({} files, +{} -{} lines)",
            commit_id, user_info.nick,
            commit_info.files_changed, commit_info.insertions, commit_info.deletions
        );

        Ok(commit_info)
    }

    /// Format a commit message from the changes and evaluated code
    fn format_commit_message(changes: &StateChanges, eval_code: &str) -> String {
        // Truncate eval code if too long
        let eval_display = if eval_code.len() > 100 {
            format!("{}...", &eval_code[..100])
        } else {
            eval_code.to_string()
        };

        let mut msg = format!("Evaluated {}", eval_display);

        // Add details about what changed
        if !changes.new_procs.is_empty() {
            msg.push_str(&format!("\n\nNew/modified procs: {}", changes.new_procs.join(", ")));
        }
        if !changes.deleted_procs.is_empty() {
            msg.push_str(&format!("\n\nDeleted procs: {}", changes.deleted_procs.join(", ")));
        }
        if !changes.new_vars.is_empty() {
            msg.push_str(&format!("\n\nNew/modified vars: {}", changes.new_vars.join(", ")));
        }
        if !changes.deleted_vars.is_empty() {
            msg.push_str(&format!("\n\nDeleted vars: {}", changes.deleted_vars.join(", ")));
        }

        msg
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

    /// Get git commit history
    /// Returns list of (commit_hash, timestamp, author, message) tuples
    pub fn get_history(&self, count: usize) -> Result<Vec<(String, i64, String, String)>> {
        self.init_git_repo_if_needed()?;
        let repo = Repository::open(&self.state_path)
            .map_err(|e| anyhow!("Failed to open git repository: {}", e))?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        for (i, oid) in revwalk.enumerate() {
            if i >= count {
                break;
            }

            let oid = oid?;
            let commit = repo.find_commit(oid)?;

            let hash = format!("{}", oid);
            let timestamp = commit.time().seconds();
            let author = commit.author().name().unwrap_or("unknown").to_string();
            let message = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();

            commits.push((hash, timestamp, author, message));
        }

        Ok(commits)
    }

    /// Rollback to a specific commit
    /// This resets HEAD to the specified commit and updates the working directory
    pub fn rollback_to(&self, commit_hash: &str) -> Result<()> {
        self.init_git_repo_if_needed()?;
        let repo = Repository::open(&self.state_path)
            .map_err(|e| anyhow!("Failed to open git repository: {}", e))?;

        // Parse the commit hash
        let oid = git2::Oid::from_str(commit_hash)
            .map_err(|e| anyhow!("Invalid commit hash: {}", e))?;

        // Find the commit
        let commit = repo.find_commit(oid)
            .map_err(|e| anyhow!("Commit not found: {}", e))?;

        // Reset to this commit (hard reset)
        repo.reset(commit.as_object(), git2::ResetType::Hard, None)
            .map_err(|e| anyhow!("Failed to reset to commit: {}", e))?;

        info!("Rolled back to commit {}", commit_hash);
        Ok(())
    }

    /// Run git gc if the commit count is a multiple of 100
    /// This prevents the repository from growing too large over time
    fn maybe_run_git_gc(&self) -> Result<()> {
        let repo = Repository::open(&self.state_path)
            .map_err(|e| anyhow!("Failed to open git repository: {}", e))?;

        // Count total commits
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        let commit_count = revwalk.count();

        // Run gc every 100 commits
        if commit_count % 100 == 0 {
            info!("Running git gc (commit count: {})", commit_count);

            // Run git gc using system command
            use std::process::Command;
            let output = Command::new("git")
                .args(&["gc", "--auto", "--quiet"])
                .current_dir(&self.state_path)
                .output()
                .map_err(|e| anyhow!("Failed to run git gc: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("git gc failed: {}", stderr);
            } else {
                debug!("git gc completed successfully");
            }
        }

        Ok(())
    }
}
