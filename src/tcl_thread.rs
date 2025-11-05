use crate::config::TclConfig;
use crate::state::{InterpreterState, StatePersistence, UserInfo};
use crate::tcl_wrapper::SafeTclInterp;
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
}

impl TclThreadHandle {
    /// Spawn a new TCL thread
    pub fn spawn(tcl_config: TclConfig, security_config: crate::config::SecurityConfig) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::channel();
        let timeout = Duration::from_millis(security_config.eval_timeout_ms);

        let thread_handle = thread::spawn(move || {
            let worker = TclThreadWorker::new(tcl_config, security_config);
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
        })
    }

    /// Evaluate TCL code with timeout
    pub async fn eval(
        &self,
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
                warn!("TCL evaluation timed out after {}ms - thread may be hung", self.timeout.as_millis());

                // TODO: Kill and restart the thread
                // For now, return an error
                Ok(EvalResult {
                    output: format!("error: evaluation timed out after {}s", self.timeout.as_secs()),
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
}

impl TclThreadWorker {
    fn new(tcl_config: TclConfig, security_config: crate::config::SecurityConfig) -> Result<Self> {
        let interp = SafeTclInterp::new(
            security_config.eval_timeout_ms,
            &tcl_config.state_path,
        )?;

        Ok(Self {
            interp,
            tcl_config,
            privileged_users: security_config.privileged_users,
        })
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
}
