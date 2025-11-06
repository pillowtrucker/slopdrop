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
}

/// Commands that can be sent to the TCL thread
pub enum TclThreadCommand {
    Eval(EvalRequest),
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
        self.command_tx
            .send(TclThreadCommand::Eval(request))
            .map_err(|e| anyhow::anyhow!("Failed to send to TCL thread: {}", e))?;

        // Wait for response with timeout
        match tokio::time::timeout(self.timeout, response_rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(anyhow::anyhow!("Response channel closed: {}", e)),
            Err(_) => {
                // Timeout! The TCL thread is hung
                warn!("TCL evaluation timed out after {}ms - thread is hung, restarting", self.timeout.as_millis());

                // Restart the thread
                if let Err(e) = self.restart() {
                    error!("Failed to restart TCL thread: {}", e);
                    return Ok(EvalResult {
                        output: format!("error: timeout and failed to restart: {}", e),
                        is_error: true,
                    });
                }

                Ok(EvalResult {
                    output: format!("error: evaluation timed out after {}s (thread restarted)", self.timeout.as_secs()),
                    is_error: true,
                })
            }
        }
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

    /// Register the chanlist command as a placeholder TCL proc
    /// The actual implementation is intercepted in handle_eval()
    fn register_chanlist_command(
        interp: &tcl::Interpreter,
        _channel_members: ChannelMembers,
    ) -> Result<()> {
        // Create a placeholder proc - actual implementation is in handle_eval
        // This ensures "chanlist" exists and can be called from TCL procs
        interp.eval(r#"
            # chanlist command - returns list of nicks in a channel
            # This is implemented in Rust and intercepted before evaluation
            proc chanlist {channel} {
                error "chanlist should have been intercepted by Rust handler"
            }
        "#).map_err(|e| anyhow::anyhow!("Failed to register chanlist command: {:?}", e))?;

        Ok(())
    }

    fn run(self, command_rx: mpsc::Receiver<TclThreadCommand>) {
        info!("TCL thread worker started");

        for command in command_rx {
            match command {
                TclThreadCommand::Eval(request) => {
                    self.handle_eval(request);
                }
                TclThreadCommand::Shutdown => {
                    info!("TCL thread worker shutting down");
                    break;
                }
            }
        }
    }

    fn handle_eval(&self, request: EvalRequest) {
        debug!("TCL thread evaluating: {}", request.code);

        // Check privilege level
        if request.is_admin && !self.privileged_users.contains(&request.nick) {
            let _ = request.response_tx.send(EvalResult {
                output: "error: tclAdmin command requires privileges".to_string(),
                is_error: true,
            });
            return;
        }

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

        // Set HTTP context variables (for rate limiting)
        // Increment eval count
        let _ = self.interp.interpreter().eval("::httpx::increment_eval");
        // Set channel context
        let set_channel = format!("set ::nick_channel {{{}}}", request.channel);
        let _ = self.interp.interpreter().eval(set_channel.as_str());

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
            },
            Err(e) => EvalResult {
                output: format!("error: {}", e),
                is_error: true,
            },
        };

        // Capture state after and save if changed
        if let Ok(state_after) = InterpreterState::capture(self.interp.interpreter()) {
            if let Ok(state_before) = state_before {
                let changes = state_before.diff(&state_after);

                if changes.has_changes() {
                    debug!("State changed: {:?}", changes);

                    let user_info = UserInfo::new(request.nick.clone(), request.host.clone());
                    let persistence = StatePersistence::new(self.tcl_config.state_path.clone());

                    if let Err(e) = persistence.save_changes(
                        self.interp.interpreter(),
                        &changes,
                        &user_info,
                        &request.code,
                    ) {
                        warn!("Failed to save state: {}", e);
                    } else {
                        debug!("State saved successfully");
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

        let persistence = StatePersistence::new(self.tcl_config.state_path.clone());

        match persistence.get_history(count) {
            Ok(commits) => {
                if commits.is_empty() {
                    let _ = request.response_tx.send(EvalResult {
                        output: "No commits found".to_string(),
                        is_error: false,
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
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
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
            });
            return;
        };

        if hash.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: rollback <commit-hash>".to_string(),
                is_error: true,
            });
            return;
        }

        let persistence = StatePersistence::new(self.tcl_config.state_path.clone());

        match persistence.rollback_to(hash) {
            Ok(()) => {
                // After rollback, state files have been reset via git
                // The TCL interpreter still has old state in memory
                // Restarting the bot (or just the TCL thread) loads fresh state from disk
                // Since rollback is an admin-only operation rarely used, manual restart is acceptable
                let _ = request.response_tx.send(EvalResult {
                    output: format!("Rolled back to commit {}. Note: Restart bot to reload state.", hash),
                    is_error: false,
                });
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: {}", e),
                    is_error: true,
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
            });
            return;
        };

        if channel.is_empty() {
            let _ = request.response_tx.send(EvalResult {
                output: "error: usage: chanlist <channel>".to_string(),
                is_error: true,
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
                        });
                    } else {
                        let mut sorted: Vec<_> = nicks.iter().cloned().collect();
                        sorted.sort();
                        let _ = request.response_tx.send(EvalResult {
                            output: sorted.join(" "),
                            is_error: false,
                        });
                    }
                } else {
                    // Channel not found - return empty list
                    let _ = request.response_tx.send(EvalResult {
                        output: String::new(),
                        is_error: false,
                    });
                }
            }
            Err(e) => {
                let _ = request.response_tx.send(EvalResult {
                    output: format!("error: failed to read channel members: {}", e),
                    is_error: true,
                });
            }
        }
    }
}
