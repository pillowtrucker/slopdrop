use crate::config::{SecurityConfig, TclConfig};
use crate::state::{InterpreterState, StatePersistence, UserInfo};
use crate::tcl_wrapper::SafeTclInterp;
use crate::types::{Message, PluginCommand};
use crate::validator;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub struct TclPlugin {
    interp: SafeTclInterp,
    security_config: SecurityConfig,
    tcl_config: TclConfig,
}

impl TclPlugin {
    pub fn new(security_config: SecurityConfig, tcl_config: TclConfig) -> Result<Self> {
        let interp = SafeTclInterp::new(
            security_config.eval_timeout_ms,
            &tcl_config.state_path,
        )?;

        Ok(Self {
            interp,
            security_config,
            tcl_config,
        })
    }

    /// Main event loop for the TCL plugin
    pub async fn run(
        &self,
        mut command_rx: mpsc::Receiver<PluginCommand>,
        response_tx: mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        info!("TCL plugin started");

        while let Some(command) = command_rx.recv().await {
            match command {
                PluginCommand::EvalTcl { message, is_admin } => {
                    if let Err(e) = self.handle_eval(message, is_admin, &response_tx).await {
                        error!("Error handling TCL eval: {}", e);
                    }
                }
                PluginCommand::Shutdown => {
                    info!("Shutting down TCL plugin");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_eval(
        &self,
        message: Message,
        is_admin: bool,
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        // Extract the command (remove "tcl " or "tclAdmin " prefix)
        let code = if message.content.starts_with("tclAdmin ") {
            message.content.strip_prefix("tclAdmin ").unwrap_or(&message.content)
        } else if message.content.starts_with("tcl ") {
            message.content.strip_prefix("tcl ").unwrap_or(&message.content)
        } else {
            &message.content
        };

        // Check privilege level
        if is_admin && !self.is_privileged_user(&message.author.nick) {
            self.send_response(
                &message,
                "error: tclAdmin command requires privileges".to_string(),
                response_tx,
            )
            .await?;
            return Ok(());
        }

        // Validate bracket balancing
        if let Err(e) = validator::validate_brackets(code) {
            self.send_response(&message, format!("error: {}", e), response_tx)
                .await?;
            return Ok(());
        }

        debug!("Evaluating TCL: {} (admin={})", code, is_admin);

        // Capture state before evaluation
        let state_before = InterpreterState::capture(self.interp.interpreter());

        // Evaluate the code
        let result = if is_admin {
            // Admin mode: direct evaluation
            self.interp.eval(code)
        } else {
            // User mode: sandboxed evaluation with context
            let mask = message
                .author
                .host
                .as_deref()
                .unwrap_or("unknown");
            self.interp.eval_with_context(
                code,
                &message.author.nick,
                mask,
                &message.author.channel,
            )
        };

        let output = match result {
            Ok(output) => output,
            Err(e) => format!("error: {}", e),
        };

        // Capture state after evaluation and check for changes
        if let Ok(state_after) = InterpreterState::capture(self.interp.interpreter()) {
            if let Ok(state_before) = state_before {
                let changes = state_before.diff(&state_after);

                if changes.has_changes() {
                    debug!("State changed: {:?}", changes);

                    // Prepare user info for git commit
                    let user_info = UserInfo::new(
                        message.author.nick.clone(),
                        message.author.host.clone().unwrap_or_else(|| "irc".to_string()),
                    );

                    // Save changes to disk and commit to git
                    let persistence = StatePersistence::new(self.tcl_config.state_path.clone());
                    if let Err(e) = persistence.save_changes(
                        self.interp.interpreter(),
                        &changes,
                        &user_info,
                        code,
                    ) {
                        warn!("Failed to save state changes: {}", e);
                    } else {
                        debug!("State changes saved and committed successfully");
                    }
                }
            }
        }

        self.send_response(&message, output, response_tx).await?;

        Ok(())
    }

    async fn send_response(
        &self,
        original_message: &Message,
        output: String,
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        // Limit output lines
        let lines: Vec<&str> = output.lines().collect();
        let output = if lines.len() > self.tcl_config.max_output_lines {
            let truncated: Vec<&str> = lines
                .iter()
                .take(self.tcl_config.max_output_lines)
                .copied()
                .collect();
            format!(
                "{}\n... ({} more lines truncated)",
                truncated.join("\n"),
                lines.len() - self.tcl_config.max_output_lines
            )
        } else {
            output
        };

        response_tx
            .send(PluginCommand::SendToIrc {
                channel: original_message.author.channel.clone(),
                text: output,
            })
            .await?;

        Ok(())
    }

    fn is_privileged_user(&self, nick: &str) -> bool {
        self.security_config.privileged_users.contains(&nick.to_string())
    }
}
