use crate::config::{SecurityConfig, TclConfig};
use crate::tcl_thread::TclThreadHandle;
use crate::types::{ChannelMembers, Message, PluginCommand};
use crate::validator;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct TclPlugin {
    tcl_thread: TclThreadHandle,
    tcl_config: TclConfig,
}

impl TclPlugin {
    pub fn new(
        security_config: SecurityConfig,
        tcl_config: TclConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let tcl_thread =
            TclThreadHandle::spawn(tcl_config.clone(), security_config, channel_members)?;

        Ok(Self {
            tcl_thread,
            tcl_config,
        })
    }

    /// Main event loop for the TCL plugin
    pub async fn run(
        &mut self,
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
        &mut self,
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

        // Validate bracket balancing
        if let Err(e) = validator::validate_brackets(code) {
            self.send_response(&message, format!("error: {}", e), response_tx)
                .await?;
            return Ok(());
        }

        debug!("Evaluating TCL: {} (admin={})", code, is_admin);

        // Send to TCL thread with timeout
        let result = self.tcl_thread.eval(
            code.to_string(),
            is_admin,
            message.author.nick.clone(),
            message.author.host.clone().unwrap_or_else(|| "irc".to_string()),
            message.author.channel.clone(),
        ).await?;

        self.send_response(&message, result.output, response_tx).await?;

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
}
