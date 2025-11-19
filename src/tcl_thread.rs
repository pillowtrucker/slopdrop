use crate::config::TclConfig;
use crate::state::{InterpreterState, StatePersistence, UserInfo};
use crate::tcl_wrapper::SafeTclInterp;
use crate::types::ChannelMembers;
use anyhow::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

#[cfg(unix)]
use nix::sys::resource::{setrlimit, Resource};

/// Set memory limit for current process (Unix only)
#[cfg(unix)]
fn set_memory_limit(limit_mb: u64) -> Result<()> {
    if limit_mb == 0 {
        // 0 means no limit
        return Ok(());
    }

    let limit_bytes = limit_mb * 1024 * 1024;

    // Set virtual memory limit (RLIMIT_AS)
    setrlimit(Resource::RLIMIT_AS, limit_bytes, limit_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to set memory limit: {}", e))?;

    info!("Memory limit set to {} MB", limit_mb);
    Ok(())
}

#[cfg(not(unix))]
fn set_memory_limit(_limit_mb: u64) -> Result<()> {
    // Memory limits not supported on non-Unix platforms
    warn!("Memory limits not supported on this platform");
    Ok(())
}

/// Request to evaluate TCL code
#[derive(Debug)]
pub struct EvalRequest {
    pub code: String,
    pub is_admin: bool,
    pub nick: String,
    pub host: String,
    pub channel: String,
    pub response_tx: oneshot::Sender<EvalResult>,
}

/// Result of TCL evaluation
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub output: String,
    /// Indicates whether the output is an error message
    /// Currently not used but kept for future error handling improvements
    #[allow(dead_code)]
    pub is_error: bool,
    /// Git commit information (if state changed and was committed)
    pub commit_info: Option<crate::state::CommitInfo>,
}

/// Commands that can be sent to the TCL thread
pub enum TclThreadCommand {
    Eval(EvalRequest),
    LogMessage {
        channel: String,
        nick: String,
        mask: String,
        text: String,
    },
    Shutdown,
}

/// Handle to communicate with the TCL thread
pub struct TclThreadHandle {
    command_tx: mpsc::Sender<TclThreadCommand>,
    thread_handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
    tcl_config: TclConfig,
    security_config: crate::config::SecurityConfig,
    channel_members: ChannelMembers,
}

impl TclThreadHandle {
    /// Spawn a new TCL thread
    pub fn spawn(
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel();
        let timeout = Duration::from_millis(security_config.eval_timeout_ms);

        let tcl_config_clone = tcl_config.clone();
        let security_config_clone = security_config.clone();
        let channel_members_clone = channel_members.clone();

        let thread_handle = thread::spawn(move || {
            // Set memory limit for this thread
            if let Err(e) = set_memory_limit(security_config_clone.memory_limit_mb) {
                error!("Failed to set memory limit: {}", e);
            }

            let worker = TclThreadWorker::new(
                tcl_config_clone,
                security_config_clone,
                channel_members_clone,
            );
            if let Err(e) = worker {
                error!("Failed to create TCL worker: {}", e);
                return;
            }

            worker.unwrap().run(command_rx);
        });

        info!("TCL thread spawned with {}ms timeout", timeout.as_millis());

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
            timeout,
            tcl_config,
            security_config,
            channel_members,
        })
    }

    /// Restart the TCL thread (called after timeout/hang)
    fn restart(&mut self) -> Result<()> {
        warn!("Restarting hung TCL thread");

        // Drop old thread handle (abandon hung thread)
        if let Some(handle) = self.thread_handle.take() {
            // Don't wait for it - it's hung
            drop(handle);
        }

        // Create new channel
        let (command_tx, command_rx) = mpsc::channel();

        // Spawn new thread
        let tcl_config = self.tcl_config.clone();
        let security_config = self.security_config.clone();
        let channel_members = self.channel_members.clone();

        let thread_handle = thread::spawn(move || {
            // Set memory limit for this thread
            if let Err(e) = set_memory_limit(security_config.memory_limit_mb) {
                error!("Failed to set memory limit after restart: {}", e);
            }

            let worker = TclThreadWorker::new(tcl_config, security_config, channel_members);
            if let Err(e) = worker {
                error!("Failed to create TCL worker after restart: {}", e);
                return;
            }

            worker.unwrap().run(command_rx);
        });

        // Update handle
        self.command_tx = command_tx;
        self.thread_handle = Some(thread_handle);

        info!("TCL thread restarted successfully");
        Ok(())
    }

    /// Evaluate TCL code with timeout
    pub async fn eval(
        &mut self,
        code: String,
        is_admin: bool,
        nick: String,
        host: String,
        channel: String,
    ) -> Result<EvalResult> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = EvalRequest {
            code,
            is_admin,
            nick,
            host,
            channel,
            response_tx,
        };

        // Send request to TCL thread
        if let Err(e) = self.command_tx.send(TclThreadCommand::Eval(request)) {
            // Channel closed - thread probably crashed/panicked
            error!("TCL thread channel closed (thread crashed): {}", e);

            // Restart the thread
            if let Err(restart_err) = self.restart() {
                error!("Failed to restart TCL thread after crash: {}", restart_err);
                return Ok(EvalResult {
                    output: format!("error: thread crashed and failed to restart: {}", restart_err),
                    is_error: true,
                    commit_info: None,
                });
            }

            return Ok(EvalResult {
                output: "error: thread crashed (likely out of memory), restarted".to_string(),
                is_error: true,
                commit_info: None,
            });
        }

        // Wait for response with timeout
        match tokio::time::timeout(self.timeout, response_rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => {
                // Response channel closed unexpectedly - thread crashed
                error!("TCL thread died unexpectedly: {}", e);

                // Restart the thread
                if let Err(restart_err) = self.restart() {
                    error!("Failed to restart TCL thread after crash: {}", restart_err);
                    return Ok(EvalResult {
                        output: format!("error: thread died and failed to restart: {}", restart_err),
                        is_error: true,
                        commit_info: None,
                    });
                }

                Ok(EvalResult {
                    output: "error: thread died unexpectedly (likely out of memory), restarted".to_string(),
                    is_error: true,
                    commit_info: None,
                })
            }
            Err(_) => {
                // Timeout! The TCL thread is hung
                warn!("TCL evaluation timed out after {}ms - thread is hung, restarting", self.timeout.as_millis());

                // Restart the thread
                if let Err(e) = self.restart() {
                    error!("Failed to restart TCL thread: {}", e);
                    return Ok(EvalResult {
                        output: format!("error: timeout and failed to restart: {}", e),
                        is_error: true,
                        commit_info: None,
                    });
                }

                Ok(EvalResult {
                    output: format!("error: evaluation timed out after {}s (thread restarted)", self.timeout.as_secs()),
                    is_error: true,
                    commit_info: None,
                })
            }
        }
    }

    /// Simple eval for system-level operations (like timer checking)
    /// Uses a "system" context without user tracking
    pub async fn eval_simple(&mut self, code: String) -> Result<String> {
        let result = self.eval(
            code,
            false,
            "system".to_string(),
            "system@bot".to_string(),
            "system".to_string(),
        ).await?;

        Ok(result.output)
    }

    /// Log a message to the channel history
    pub fn log_message(&self, channel: String, nick: String, mask: String, text: String) {
        let _ = self.command_tx.send(TclThreadCommand::LogMessage {
            channel,
            nick,
            mask,
            text,
        });
    }

    /// Shutdown the TCL thread
    pub fn shutdown(&mut self) {
        info!("Shutting down TCL thread");
        let _ = self.command_tx.send(TclThreadCommand::Shutdown);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TclThreadHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Worker that runs in the TCL thread
struct TclThreadWorker {
    interp: SafeTclInterp,
    tcl_config: TclConfig,
    privileged_users: Vec<String>,
    channel_members: ChannelMembers,
}

impl TclThreadWorker {
    fn new(
        tcl_config: TclConfig,
        security_config: crate::config::SecurityConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let interp = SafeTclInterp::new(
            security_config.eval_timeout_ms,
            &tcl_config.state_path,
            tcl_config.state_repo.clone(),
            tcl_config.ssh_key.clone(),
            security_config.max_recursion_depth,
        )?;

        // Register chanlist command
        Self::register_chanlist_command(interp.interpreter(), channel_members.clone())?;

        Ok(Self {
            interp,
            tcl_config,
            privileged_users: security_config.privileged_users,
            channel_members,
        })
    }

    /// Register the chanlist command that reads from the synced channel members array
    fn register_chanlist_command(
        interp: &tcl::Interpreter,
        _channel_members: ChannelMembers,
    ) -> Result<()> {
        // Create a proc that reads from the ::slopdrop_channel_members array
        // This array is synced before each eval by sync_channel_members()
        interp.eval(r#"
            # chanlist command - returns list of nicks in a channel
            # Reads from ::slopdrop_channel_members which is synced before each eval
            proc chanlist {channel} {
                if {[info exists ::slopdrop_channel_members($channel)]} {
                    return $::slopdrop_channel_members($channel)
                } else {
                    return ""
                }
            }
        "#).map_err(|e| anyhow::anyhow!("Failed to register chanlist command: {:?}", e))?;

        Ok(())
    }

    /// Sync channel members from Rust to TCL global array
    fn sync_channel_members(&self) {
        match self.channel_members.read() {
            Ok(members) => {
                for (channel, nicks) in members.iter() {
                    if !nicks.is_empty() {
                        let mut sorted: Vec<_> = nicks.iter().cloned().collect();
                        sorted.sort();

                        // Escape channel name and nicks for TCL
                        let escaped_channel = channel
                            .replace('\\', "\\\\")
                            .replace('{', "\\{")
                            .replace('}', "\\}");

                        let escaped_nicks: Vec<String> = sorted.iter()
                            .map(|n| n
                                .replace('\\', "\\\\")
                                .replace('{', "\\{")
                                .replace('}', "\\}"))
                            .collect();

                        let tcl_code = format!(
                            "set ::slopdrop_channel_members({}) {{{}}}",
                            escaped_channel,
                            escaped_nicks.join(" ")
                        );

                        if let Err(e) = self.interp.interpreter().eval(tcl_code.as_str()) {
                            warn!("Failed to sync channel members for {}: {:?}", channel, e);
                        }
                    } else {
                        // Empty channel - unset if exists
                        let escaped_channel = channel
                            .replace('\\', "\\\\")
                            .replace('{', "\\{")
                            .replace('}', "\\}");
                        let unset_cmd = format!(
                            "catch {{unset ::slopdrop_channel_members({})}}",
                            escaped_channel
                        );
                        let _ = self.interp.interpreter().eval(unset_cmd.as_str());
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read channel members: {:?}", e);
            }
        }
    }

    fn run(self, command_rx: mpsc::Receiver<TclThreadCommand>) {
        info!("TCL thread worker started");

        for command in command_rx {
            match command {
                TclThreadCommand::Eval(request) => {
                    self.handle_eval(request);
                }
                TclThreadCommand::LogMessage { channel, nick, mask, text } => {
                    self.handle_log_message(channel, nick, mask, text);
                }
                TclThreadCommand::Shutdown => {
                    info!("TCL thread worker shutting down");
                    break;
                }
            }
        }
    }

    fn handle_log_message(&self, channel: String, nick: String, mask: String, text: String) {
        // Store message in ::slopdrop_log_lines($channel)
        // Format: {timestamp nick mask message}
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Escape TCL special characters in text
        let escaped_text = text
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('$', "\\$")
            .replace('"', "\\\"");

        let escaped_nick = nick
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        let escaped_mask = mask
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        let escaped_channel = channel
            .replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}");

        // Add to log array with size limit (default 1000 lines per channel)
        let tcl_code = format!(r#"
            set entry [list {} {{{}}} {{{}}} {{{}}}]
            if {{![info exists ::slopdrop_log_lines({})}} {{
                set ::slopdrop_log_lines({}) [list]
            }}
            lappend ::slopdrop_log_lines({}) $entry
            # Keep only last 1000 entries
            if {{[llength $::slopdrop_log_lines({})] > 1000}} {{
                set ::slopdrop_log_lines({}) [lrange $::slopdrop_log_lines({}) end-999 end]
            }}
        "#,
            timestamp, escaped_nick, escaped_mask, escaped_text,
            escaped_channel, escaped_channel, escaped_channel,
            escaped_channel, escaped_channel, escaped_channel
        );

        if let Err(e) = self.interp.interpreter().eval(tcl_code.as_str()) {
            debug!("Failed to log message: {:?}", e);
        }
    }

    fn handle_eval(&self, request: EvalRequest) {
        debug!("TCL thread evaluating: {}", request.code);

        // Check privilege level using hostmask matching
        if request.is_admin {
            // Build full hostmask: nick!ident@host
            // host parameter contains "ident@host" as built in tcl_plugin
            let hostmask = format!("{}!{}", request.nick, request.host);

            // Check if hostmask matches any privileged pattern
            let is_privileged = self.privileged_users.iter().any(|pattern| {
                crate::hostmask::matches_hostmask(&hostmask, pattern)
            });

            if !is_privileged {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: tclAdmin requires privileges (your hostmask: {})", hostmask),
                    is_error: true,
                    commit_info: None,
                });
                return;
            }
        }

        // Get eval count for rate limiting (needed for all commands)
        let eval_count_result = self.interp.interpreter().eval("::httpx::increment_eval");
        let eval_count = eval_count_result
            .ok()
            .and_then(|obj| obj.get_string().parse::<u64>().ok())
            .unwrap_or(0);

        // Set HTTP context variables (for rate limiting)
        let set_channel = format!("set ::nick_channel {{{}}}", request.channel);
        let _ = self.interp.interpreter().eval(set_channel.as_str());

        // Set stock context for rate limiting
        crate::stock_commands::set_stock_context(request.nick.clone(), eval_count);

        // Sync channel members to TCL array before evaluation
        self.sync_channel_members();

        // Check for special commands
        let code_trimmed = request.code.trim();
        if code_trimmed == "history" || code_trimmed.starts_with("history ") {
            self.handle_history_command(request);
            return;
        }
        if code_trimmed.starts_with("rollback ") {
            self.handle_rollback_command(request);
            return;
        }
        if code_trimmed.starts_with("chanlist ") {
            self.handle_chanlist_command(request);
            return;
        }
        if code_trimmed.starts_with("stock::") {
            self.handle_stock_command(request);
            return;
        }

        // Capture state before evaluation
        let state_before = InterpreterState::capture(self.interp.interpreter());

        // Evaluate the code
        let result = if request.is_admin {
            self.interp.eval(&request.code)
        } else {
            self.interp.eval_with_context(
                &request.code,
                &request.nick,
                &request.host,
                &request.channel,
            )
        };

        let output = match result {
            Ok(output) => EvalResult {
                output,
                is_error: false,
                commit_info: None,
            },
            Err(e) => EvalResult {
                output: format!("error: {}", e),
                is_error: true,
                commit_info: None,
            },
        };

        // Capture state after and save if changed
        let mut output = output;
        if let Ok(state_after) = InterpreterState::capture(self.interp.interpreter()) {
            if let Ok(state_before) = state_before {
                let changes = state_before.diff(&state_after);

                if changes.has_changes() {
                    debug!("State changed: {:?}", changes);

                    let user_info = UserInfo::new(request.nick.clone(), request.host.clone());
                    let persistence = StatePersistence::with_repo(
                        self.tcl_config.state_path.clone(),
                        self.tcl_config.state_repo.clone(),
                        self.tcl_config.ssh_key.clone(),
                    );

                    match persistence.save_changes(
                        self.interp.interpreter(),
                        &changes,
                        &user_info,
                        &request.code,
                    ) {
                        Ok(commit_info) => {
                            debug!("State saved successfully");
                            output.commit_info = commit_info;
                        }
                        Err(e) => {
                            warn!("Failed to save state: {}", e);
                        }
                    }
                }
            }
        }

        // Send response back
        let _ = request.response_tx.send(output);
    }

    fn handle_history_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Parse count from "history" or "history <count>"
        let count = if code == "history" {
            10 // default
        } else if let Some(count_str) = code.strip_prefix("history ") {
            count_str.trim().parse::<usize>().unwrap_or(10)
        } else {
            10
        };

        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        match persistence.get_history(count) {
            Ok(commits) => {
                if commits.is_empty() {
                    let _ = request.response_tx.send(EvalResult {
                        output: "No commits found".to_string(),
                        is_error: false,
                        commit_info: None,
                    });
                    return;
                }

                // Format commits as TCL list
                let mut output = String::new();
                for (hash, timestamp, author, message) in commits {
                    // Format: {hash timestamp author message}
                    let date = chrono::DateTime::from_timestamp(timestamp, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| timestamp.to_string());

                    output.push_str(&format!("{} {} {} {}\n",
                        &hash[..8], date, author, message));
                }

                let _ = request.response_tx.send(EvalResult {
                    output: output.trim_end().to_string(),
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_rollback_command(&self, request: EvalRequest) {
        // Rollback is admin-only
        if !request.is_admin {
            let _ = request.response_tx.send(EvalResult {
                output: "error: rollback requires admin privileges (use tclAdmin)".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        let code = request.code.trim();

        // Parse commit hash from "rollback <hash>"
        let hash = if let Some(h) = code.strip_prefix("rollback ") {
            h.trim()
        } else {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: rollback <commit-hash>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        };

        if hash.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: rollback <commit-hash>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        let persistence = StatePersistence::with_repo(
            self.tcl_config.state_path.clone(),
            self.tcl_config.state_repo.clone(),
            self.tcl_config.ssh_key.clone(),
        );

        match persistence.rollback_to(hash) {
            Ok(()) => {
                // After rollback, state files have been reset via git
                // The TCL interpreter still has old state in memory
                // Restarting the bot (or just the TCL thread) loads fresh state from disk
                // Since rollback is an admin-only operation rarely used, manual restart is acceptable
                let _ = request.response_tx.send(EvalResult {
                    output: format!("Rolled back to commit {}. Note: Restart bot to reload state.", hash),
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_chanlist_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Parse channel from "chanlist <channel>"
        let channel = if let Some(ch) = code.strip_prefix("chanlist ") {
            ch.trim()
        } else {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: chanlist <channel>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        };

        if channel.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: chanlist <channel>".to_string(),
                is_error: true,
                commit_info: None,
            });
            return;
        }

        // Read from shared channel members
        match self.channel_members.read() {
            Ok(members) => {
                if let Some(nicks) = members.get(channel) {
                    if nicks.is_empty() {
                        let _ = request.response_tx.send(EvalResult {
                            output: String::new(),
                            is_error: false,
                            commit_info: None,
                        });
                    } else {
                        let mut sorted: Vec<_> = nicks.iter().cloned().collect();
                        sorted.sort();
                        let _ = request.response_tx.send(EvalResult {
                            output: sorted.join(" "),
                            is_error: false,
                            commit_info: None,
                        });
                    }
                } else {
                    // Channel not found - return empty list
                    let _ = request.response_tx.send(EvalResult {
                        output: String::new(),
                        is_error: false,
                        commit_info: None,
                    });
                }
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: failed to read channel members: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }

    fn handle_stock_command(&self, request: EvalRequest) {
        let code = request.code.trim();

        // Call the stock command handler from stock_commands module
        match crate::stock_commands::handle_stock_command(code) {
            Ok(output) => {
                let _ = request.response_tx.send(EvalResult {
                    output,
                    is_error: false,
                    commit_info: None,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
                    commit_info: None,
                });
            }
        }
    }
}
