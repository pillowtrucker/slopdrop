//! CLI/REPL frontend for slopdrop
//!
//! Provides an interactive command-line interface for TCL evaluation

use crate::config::{SecurityConfig, TclConfig};
use crate::frontend::Frontend;
use crate::tcl_service::{EvalContext, TclService};
use crate::types::ChannelMembers;
use anyhow::{Context, Result};
use async_trait::async_trait;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustylineResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{error, info};

/// CLI frontend configuration
#[derive(Clone, Debug)]
pub struct CliConfig {
    /// Prompt to display
    pub prompt: String,
    /// History file path
    pub history_file: Option<String>,
    /// Username for evaluation context
    pub username: String,
    /// Whether the user has admin privileges
    pub is_admin: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            prompt: "slopdrop> ".to_string(),
            history_file: Some(".slopdrop_history".to_string()),
            username: whoami::username(),
            is_admin: true, // CLI users are typically admins
        }
    }
}

/// CLI frontend implementation
pub struct CliFrontend {
    name: String,
    config: CliConfig,
    tcl_service: TclService,
    running: bool,
}

impl CliFrontend {
    /// Create a new CLI frontend
    pub fn new(
        config: CliConfig,
        security_config: SecurityConfig,
        tcl_config: TclConfig,
    ) -> Result<Self> {
        let channel_members = Arc::new(RwLock::new(HashMap::new()));
        let tcl_service = TclService::new(security_config, tcl_config, channel_members)?;

        Ok(Self {
            name: "CLI".to_string(),
            config,
            tcl_service,
            running: false,
        })
    }

    /// Run the REPL loop
    async fn run_repl(&mut self) -> Result<()> {
        let mut rl = DefaultEditor::new().context("Failed to create readline editor")?;

        // Load history if available
        if let Some(ref history_file) = self.config.history_file {
            let _ = rl.load_history(history_file);
        }

        println!("Welcome to Slopdrop TCL Evalbot");
        println!("Type '.help' for help, '.quit' to exit");
        println!();

        while self.running {
            let readline = rl.readline(&self.config.prompt);

            match readline {
                Ok(line) => {
                    let line = line.trim();

                    if line.is_empty() {
                        continue;
                    }

                    // Add to history
                    rl.add_history_entry(line)
                        .map_err(|e| anyhow::anyhow!("Failed to add history: {}", e))?;

                    // Handle special commands
                    if line.starts_with('.') {
                        if let Err(e) = self.handle_special_command(line).await {
                            eprintln!("Error: {}", e);
                        }
                        continue;
                    }

                    // Evaluate TCL code
                    let ctx = EvalContext::new(self.config.username.clone(), "local".to_string())
                        .with_admin(self.config.is_admin);

                    match self.tcl_service.eval(line, ctx).await {
                        Ok(response) => {
                            for line in &response.output {
                                println!("{}", line);
                            }

                            if response.more_available {
                                println!("... (more lines available - type '.more' to continue)");
                            }

                            if let Some(commit_info) = response.commit_info {
                                println!(
                                    "[Git] {} | {} files changed (+{} -{})",
                                    &commit_info.commit_id[..8],
                                    commit_info.files_changed,
                                    commit_info.insertions,
                                    commit_info.deletions
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("^D");
                    break;
                }
                Err(err) => {
                    error!("Readline error: {}", err);
                    break;
                }
            }
        }

        // Save history
        if let Some(ref history_file) = self.config.history_file {
            let _ = rl.save_history(history_file);
        }

        Ok(())
    }

    /// Handle special commands (.help, .quit, etc.)
    async fn handle_special_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let cmd = parts[0];

        match cmd {
            ".help" => {
                println!("Available commands:");
                println!("  <tcl code>      - Evaluate TCL code");
                println!("  .help           - Show this help");
                println!("  .quit / .exit   - Exit the REPL");
                println!("  .history [N]    - Show git commit history (last N commits)");
                println!("  .rollback <hash> - Rollback to a specific commit");
                println!("  .more           - Show more paginated output");
            }
            ".quit" | ".exit" => {
                self.running = false;
            }
            ".history" => {
                let limit = if parts.len() > 1 {
                    parts[1].parse().unwrap_or(10)
                } else {
                    10
                };

                match self.tcl_service.history(limit).await {
                    Ok(commits) => {
                        if commits.is_empty() {
                            println!("No commit history available");
                        } else {
                            println!("Git History:");
                            for commit in commits {
                                println!(
                                    "  {} - {} - {} ({} files, +{} -{})",
                                    &commit.commit_id[..8],
                                    commit.author,
                                    commit.message.lines().next().unwrap_or(""),
                                    commit.files_changed,
                                    commit.insertions,
                                    commit.deletions
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to get history: {}", e);
                    }
                }
            }
            ".rollback" => {
                if parts.len() < 2 {
                    eprintln!("Usage: .rollback <commit-hash>");
                    return Ok(());
                }

                let commit_hash = parts[1];
                match self.tcl_service.rollback(commit_hash).await {
                    Ok(message) => {
                        println!("{}", message);
                    }
                    Err(e) => {
                        eprintln!("Failed to rollback: {}", e);
                    }
                }
            }
            ".more" => {
                let ctx = EvalContext::new(self.config.username.clone(), "local".to_string())
                    .with_admin(self.config.is_admin);

                match self.tcl_service.more(ctx).await {
                    Ok(response) => {
                        for line in &response.output {
                            println!("{}", line);
                        }

                        if response.more_available {
                            println!("... (more lines available - type '.more' to continue)");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            _ => {
                eprintln!("Unknown command: {}. Type '.help' for help.", cmd);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Frontend for CliFrontend {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting CLI frontend");
        self.running = true;
        self.run_repl().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping CLI frontend");
        self.running = false;
        self.tcl_service.shutdown();
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }
}
