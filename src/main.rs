//! Multi-frontend main entry point
//!
//! Supports running multiple frontends (IRC, CLI, TUI, Web) simultaneously

mod config;
mod hostmask;
mod http_commands;
mod http_tcl_commands;
mod irc_client;
mod irc_formatting;
mod smeggdrop_commands;
mod state;
mod tcl_plugin;
mod tcl_thread;
mod tcl_wrapper;
mod types;
mod validator;

// Multi-frontend support
mod frontend;
mod tcl_service;
mod frontends;

use anyhow::Result;
use config::Config;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio::signal;
// warn! is used in conditional compilation blocks when features are not enabled
#[allow(unused_imports)]
use tracing::{error, info, warn};
use tracing_subscriber;

#[derive(Debug)]
struct FrontendFlags {
    irc: bool,
    cli: bool,
    tui: bool,
    web: bool,
}

impl FrontendFlags {
    fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();

        // If no frontend flags specified, default to IRC
        if !args.iter().any(|a| a.starts_with("--")) {
            return Self {
                irc: true,
                cli: false,
                tui: false,
                web: false,
            };
        }

        Self {
            irc: args.contains(&"--irc".to_string()),
            cli: args.contains(&"--cli".to_string()),
            tui: args.contains(&"--tui".to_string()),
            web: args.contains(&"--web".to_string()),
        }
    }

    fn any_enabled(&self) -> bool {
        self.irc || self.cli || self.tui || self.web
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Slopdrop TCL evalbot starting");

    // Parse frontend flags
    let flags = FrontendFlags::from_args();
    info!("Frontend flags: {:?}", flags);

    if !flags.any_enabled() {
        error!("No frontends enabled! Use --irc, --cli, --tui, or --web");
        println!("Usage: slopdrop [config.toml] [--irc] [--cli] [--tui] [--web]");
        println!();
        println!("Frontends:");
        println!("  --irc    IRC bot (default if no flags specified)");
        println!("  --cli    Interactive command-line REPL");
        println!("  --tui    Full-screen terminal UI");
        println!("  --web    Web server with HTTP API");
        println!();
        println!("Examples:");
        println!("  slopdrop                    # IRC bot (default)");
        println!("  slopdrop --cli              # CLI REPL only");
        println!("  slopdrop --tui              # TUI only");
        println!("  slopdrop --web              # Web server only");
        println!("  slopdrop --irc --web        # Both IRC and Web");
        println!("  slopdrop --cli --tui --web  # All except IRC");
        return Ok(());
    }

    // Find config file path
    let config_path = std::env::args()
        .find(|arg| !arg.starts_with("--") && arg.ends_with(".toml"))
        .unwrap_or_else(|| "config.toml".to_string());

    // Load configuration
    let config = match Config::from_file(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load config from {}: {}", config_path, e);
            error!("Please create a config.toml file (see config.toml.example)");
            return Err(e);
        }
    };

    info!("Configuration loaded from {}", config_path);

    // Start requested frontends
    let mut tasks = vec![];

    // CLI Frontend
    #[cfg(feature = "frontend-cli")]
    if flags.cli {
        info!("Starting CLI frontend");
        let security_config = config.security.clone();
        let tcl_config = config.tcl.clone();

        let task = tokio::spawn(async move {
            use crate::frontend::Frontend;
            use crate::frontends::cli::{CliConfig, CliFrontend};

            let cli_config = CliConfig::default();
            match CliFrontend::new(cli_config, security_config, tcl_config) {
                Ok(mut frontend) => {
                    if let Err(e) = frontend.start().await {
                        error!("CLI frontend error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create CLI frontend: {}", e);
                }
            }
        });
        tasks.push(task);
    }
    #[cfg(not(feature = "frontend-cli"))]
    if flags.cli {
        warn!("CLI frontend requested but not compiled! Build with --features frontend-cli");
    }

    // TUI Frontend
    #[cfg(feature = "frontend-tui")]
    if flags.tui {
        info!("Starting TUI frontend");
        let security_config = config.security.clone();
        let tcl_config = config.tcl.clone();

        let task = tokio::spawn(async move {
            use crate::frontend::Frontend;
            use crate::frontends::tui::{TuiConfig, TuiFrontend};

            let tui_config = TuiConfig::default();
            match TuiFrontend::new(tui_config, security_config, tcl_config) {
                Ok(mut frontend) => {
                    if let Err(e) = frontend.start().await {
                        error!("TUI frontend error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create TUI frontend: {}", e);
                }
            }
        });
        tasks.push(task);
    }
    #[cfg(not(feature = "frontend-tui"))]
    if flags.tui {
        warn!("TUI frontend requested but not compiled! Build with --features frontend-tui");
    }

    // Web Frontend
    #[cfg(feature = "frontend-web")]
    if flags.web {
        info!("Starting Web frontend");
        let security_config = config.security.clone();
        let tcl_config = config.tcl.clone();

        let task = tokio::spawn(async move {
            use crate::frontend::Frontend;
            use crate::frontends::web::{WebConfig, WebFrontend};

            let web_config = WebConfig::default();
            match WebFrontend::new(web_config, security_config, tcl_config) {
                Ok(mut frontend) => {
                    if let Err(e) = frontend.start().await {
                        error!("Web frontend error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create Web frontend: {}", e);
                }
            }
        });
        tasks.push(task);
    }
    #[cfg(not(feature = "frontend-web"))]
    if flags.web {
        warn!("Web frontend requested but not compiled! Build with --features frontend-web");
    }

    // IRC Frontend (using existing code)
    if flags.irc {
        info!("Starting IRC frontend");

        // Create shared channel members tracking
        let channel_members = Arc::new(RwLock::new(HashMap::new()));

        // Create communication channels
        let (tcl_command_tx, tcl_command_rx) = mpsc::channel(100);
        let (irc_response_tx, irc_response_rx) = mpsc::channel(100);

        // Spawn TCL plugin task
        let tcl_handle = {
            let security_config = config.security.clone();
            let tcl_config = config.tcl.clone();
            let channel_members_clone = channel_members.clone();
            tokio::task::spawn_blocking(move || {
                let mut tcl_plugin = match tcl_plugin::TclPlugin::new(
                    security_config,
                    tcl_config,
                    channel_members_clone,
                ) {
                    Ok(plugin) => plugin,
                    Err(e) => {
                        error!("Failed to create TCL plugin: {}", e);
                        return;
                    }
                };

                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    if let Err(e) = tcl_plugin.run(tcl_command_rx, irc_response_tx).await {
                        error!("TCL plugin error: {}", e);
                    }
                });
            })
        };

        // Create and run IRC client
        let irc_task = tokio::spawn(async move {
            match irc_client::IrcClient::new(config.server.clone(), channel_members).await {
                Ok(irc_client) => {
                    info!("Joining channels: {:?}", config.server.channels);
                    if let Err(e) = irc_client.run(tcl_command_tx, irc_response_rx).await {
                        error!("IRC client error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create IRC client: {}", e);
                }
            }
        });

        tasks.push(irc_task);
        tasks.push(tcl_handle);
    }

    // Wait for shutdown signal or task completion
    info!("All frontends started. Press Ctrl+C to stop.");

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
        }
        _ = wait_for_tasks(tasks) => {
            info!("All tasks completed");
        }
    }

    info!("Slopdrop shut down successfully");
    Ok(())
}

/// Wait for any task to complete
async fn wait_for_tasks(tasks: Vec<tokio::task::JoinHandle<()>>) {
    for task in tasks {
        if let Err(e) = task.await {
            error!("Task error: {}", e);
        }
    }
}
