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

use anyhow::Result;
use config::Config;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Slopdrop TCL evalbot starting");

    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    let config = match Config::from_file(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load config from {}: {}", config_path, e);
            error!("Please create a config.toml file (see config.toml.example)");
            return Err(e);
        }
    };

    info!("Configuration loaded from {}", config_path);

    // Create shared channel members tracking
    let channel_members = Arc::new(RwLock::new(HashMap::new()));

    // Create communication channels
    // IRC -> TCL plugin
    let (tcl_command_tx, tcl_command_rx) = mpsc::channel(100);
    // TCL plugin -> IRC
    let (irc_response_tx, irc_response_rx) = mpsc::channel(100);

    // Spawn TCL plugin task
    // Note: TCL interpreter must be created inside the task since it's not Send
    let tcl_handle = {
        let security_config = config.security.clone();
        let tcl_config = config.tcl.clone();
        let channel_members_clone = channel_members.clone();
        tokio::task::spawn_blocking(move || {
            // Create TCL plugin within the thread
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

            // Run the plugin (blocking)
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                if let Err(e) = tcl_plugin.run(tcl_command_rx, irc_response_tx).await {
                    error!("TCL plugin error: {}", e);
                }
            });
        })
    };

    // Create and run IRC client
    let irc_client = irc_client::IrcClient::new(config.server.clone(), channel_members).await?;

    info!("Joining channels: {:?}", config.server.channels);

    // Run IRC client (this blocks until shutdown)
    let irc_result = irc_client.run(tcl_command_tx, irc_response_rx).await;

    // Wait for TCL plugin to finish
    tcl_handle.await?;

    match irc_result {
        Ok(_) => {
            info!("Slopdrop shut down successfully");
            Ok(())
        }
        Err(e) => {
            error!("IRC client error: {}", e);
            Err(e)
        }
    }
}
