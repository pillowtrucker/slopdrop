use crate::config::{Config, SecurityConfig, ServerConfig, TclConfig};
use crate::file_watcher::{ChangeType, FileChangeEvent};
use crate::hostmask;
use crate::tcl_thread::TclThreadHandle;
use crate::types::{ChannelMembers, Message, PluginCommand};
use crate::validator;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Cache entry for paginated output
struct OutputCache {
    lines: Vec<String>,
    offset: usize,
    timestamp: Instant,
}

pub struct TclPlugin {
    tcl_thread: TclThreadHandle,
    tcl_config: TclConfig,
    security_config: SecurityConfig,
    server_configs: Vec<ServerConfig>,
    /// Path to configuration file for hot-reloading
    config_path: std::path::PathBuf,
    /// Cache for paginated output: (channel, nick) -> remaining output
    output_cache: HashMap<(String, String), OutputCache>,
    /// Nicks of currently online admins (updated on join/part/quit)
    admin_nicks: HashSet<String>,
    /// Per-network response channels for routing responses to correct IRC client
    response_channels: HashMap<String, mpsc::Sender<PluginCommand>>,
}

impl TclPlugin {
    pub fn new(
        security_config: SecurityConfig,
        tcl_config: TclConfig,
        server_configs: Vec<ServerConfig>,
        config_path: std::path::PathBuf,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let tcl_thread =
            TclThreadHandle::spawn(tcl_config.clone(), security_config.clone(), channel_members)?;

        Ok(Self {
            tcl_thread,
            tcl_config,
            security_config,
            server_configs,
            config_path,
            output_cache: HashMap::new(),
            admin_nicks: HashSet::new(),
            response_channels: HashMap::new(),
        })
    }

    /// Register a response channel for a network
    pub fn register_network(&mut self, network_name: String, response_tx: mpsc::Sender<PluginCommand>) {
        info!("Registered response channel for network: {}", network_name);
        self.response_channels.insert(network_name, response_tx);
    }

    /// Send a response to the correct network's IRC client
    async fn send_to_network(&self, network: &str, command: PluginCommand) -> Result<()> {
        if let Some(tx) = self.response_channels.get(network) {
            tx.send(command).await?;
        } else {
            warn!("No response channel for network '{}', dropping message", network);
        }
        Ok(())
    }

    /// Main event loop for the TCL plugin
    pub async fn run(
        &mut self,
        mut command_rx: mpsc::Receiver<PluginCommand>,
        file_change_rx: Option<std::sync::mpsc::Receiver<FileChangeEvent>>,
    ) -> Result<()> {
        info!("TCL plugin started");

        // Timer polling interval (1 second)
        let mut timer_interval = interval(Duration::from_secs(1));

        loop {
            // Check for file changes (non-blocking) and batch them
            if let Some(ref rx) = file_change_rx {
                let mut has_tcl_changes = false;
                let mut has_config_changes = false;

                // Drain all pending events to batch reloads
                while let Ok(event) = rx.try_recv() {
                    debug!("File change detected: {:?} ({:?})", event.path, event.change_type);
                    match event.change_type {
                        ChangeType::TclModule => has_tcl_changes = true,
                        ChangeType::Config => has_config_changes = true,
                    }
                }

                // Process batched changes - only reload once even if multiple files changed
                if has_tcl_changes {
                    info!("Reloading TCL modules due to file changes");
                    self.tcl_thread.reload();
                }
                if has_config_changes {
                    info!("Reloading configuration from disk");
                    if let Err(e) = self.reload_config() {
                        error!("Failed to reload configuration: {}", e);
                    }
                }
            }

            tokio::select! {
                // Handle incoming commands
                command = command_rx.recv() => {
                    match command {
                        Some(PluginCommand::EvalTcl { message, is_admin }) => {
                            if let Err(e) = self.handle_eval(message, is_admin).await {
                                error!("Error handling TCL eval: {}", e);
                            }
                        }
                        Some(PluginCommand::LogMessage { channel, nick, mask, text, .. }) => {
                            self.tcl_thread.log_message(channel, nick, mask, text);
                        }
                        Some(PluginCommand::UserJoin { network, channel, nick, mask }) => {
                            // Track admin status on join
                            self.update_admin_status(&nick, &mask, true);
                            if let Err(e) = self.handle_event("JOIN", &[&nick, &mask, &channel], &network).await {
                                warn!("Error handling JOIN event: {}", e);
                            }
                        }
                        Some(PluginCommand::UserPart { network, channel, nick, mask }) => {
                            // Remove from admin list on part
                            self.admin_nicks.remove(&nick);
                            if let Err(e) = self.handle_event("PART", &[&nick, &mask, &channel], &network).await {
                                warn!("Error handling PART event: {}", e);
                            }
                        }
                        Some(PluginCommand::UserQuit { network, nick, mask, message }) => {
                            // Remove from admin list on quit
                            self.admin_nicks.remove(&nick);
                            if let Err(e) = self.handle_event("QUIT", &[&nick, &mask, &message], &network).await {
                                warn!("Error handling QUIT event: {}", e);
                            }
                        }
                        Some(PluginCommand::UserKick { network, channel, nick, kicker, reason }) => {
                            // Remove kicked user from admin list
                            self.admin_nicks.remove(&nick);
                            if let Err(e) = self.handle_event("KICK", &[&nick, &kicker, &channel, &reason], &network).await {
                                warn!("Error handling KICK event: {}", e);
                            }
                        }
                        Some(PluginCommand::UserNick { network, old_nick, new_nick, mask }) => {
                            // Update admin tracking for nick change
                            if self.admin_nicks.remove(&old_nick) {
                                self.admin_nicks.insert(new_nick.clone());
                            } else {
                                // Check if new hostmask is admin
                                self.update_admin_status(&new_nick, &mask, true);
                            }
                            if let Err(e) = self.handle_event("NICK", &[&old_nick, &new_nick, &mask], &network).await {
                                warn!("Error handling NICK event: {}", e);
                            }
                        }
                        Some(PluginCommand::UserHostChange { network: _, nick, old_mask: _, new_mask }) => {
                            // Re-check admin status with new hostmask
                            self.admin_nicks.remove(&nick);
                            self.update_admin_status(&nick, &new_mask, true);
                            debug!("Updated admin status for {} after host change", nick);
                        }
                        Some(PluginCommand::UserText { network, channel, nick, mask, text }) => {
                            // Update admin status on every message in case host changed
                            if !self.admin_nicks.contains(&nick) {
                                self.update_admin_status(&nick, &mask, true);
                            }
                            if let Err(e) = self.handle_event("TEXT", &[&nick, &mask, &channel, &text], &network).await {
                                warn!("Error handling TEXT event: {}", e);
                            }
                        }
                        Some(PluginCommand::Shutdown) => {
                            info!("Shutting down TCL plugin");
                            break;
                        }
                        Some(_) => {}
                        None => {
                            // Channel closed
                            break;
                        }
                    }
                }
                // Poll timers periodically
                _ = timer_interval.tick() => {
                    if let Err(e) = self.check_timers().await {
                        warn!("Error checking timers: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Reload configuration from disk
    fn reload_config(&mut self) -> Result<()> {
        // Load new config from file
        let new_config = Config::from_file(
            self.config_path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid config path"))?
        )?;

        info!("Configuration reloaded successfully");

        let new_servers = new_config.get_servers();

        // Compare server configs
        for (i, new_srv) in new_servers.iter().enumerate() {
            let old_srv = self.server_configs.get(i);
            if let Some(old) = old_srv {
                if old.hostname != new_srv.hostname {
                    warn!("  Server {}: hostname {} -> {} (requires restart/reconnect)",
                        new_srv.network_name(), old.hostname, new_srv.hostname);
                }
                if old.port != new_srv.port {
                    warn!("  Server {}: port {} -> {} (requires restart/reconnect)",
                        new_srv.network_name(), old.port, new_srv.port);
                }
            }
        }

        // Security config changes
        if self.security_config.eval_timeout_ms != new_config.security.eval_timeout_ms {
            info!("  Timeout: {}ms -> {}ms",
                self.security_config.eval_timeout_ms,
                new_config.security.eval_timeout_ms);
        }

        if self.security_config.privileged_users != new_config.security.privileged_users {
            info!("  Privileged users: {} -> {} patterns",
                self.security_config.privileged_users.len(),
                new_config.security.privileged_users.len());
        }

        if self.security_config.blacklisted_users != new_config.security.blacklisted_users {
            info!("  Blacklisted users: {} -> {} patterns",
                self.security_config.blacklisted_users.len(),
                new_config.security.blacklisted_users.len());
        }

        if self.security_config.notify_self != new_config.security.notify_self {
            info!("  Notify self: {} -> {}",
                self.security_config.notify_self,
                new_config.security.notify_self);
        }

        // TCL config changes
        if self.tcl_config.max_output_lines != new_config.tcl.max_output_lines {
            info!("  Max output lines: {} -> {}",
                self.tcl_config.max_output_lines,
                new_config.tcl.max_output_lines);
        }

        // Update our configs
        self.server_configs = new_servers;
        self.security_config = new_config.security.clone();
        self.tcl_config = new_config.tcl.clone();

        // Update the TCL thread's configuration
        self.tcl_thread.update_config(new_config.tcl, new_config.security)?;

        Ok(())
    }

    /// Handle an IRC event and dispatch to registered triggers
    async fn handle_event(
        &mut self,
        event: &str,
        args: &[&str],
        network: &str,
    ) -> Result<()> {
        // Build TCL command to dispatch event (include network)
        let tcl_args: Vec<String> = args.iter().map(|s| format!("{{{}}}", s)).collect();
        let dispatch_cmd = format!("triggers dispatch {} {{{}}} {}", event, network, tcl_args.join(" "));

        debug!("Dispatching event: {}", dispatch_cmd);

        // Evaluate the dispatch command
        let result = self.tcl_thread.eval_simple(dispatch_cmd).await?;

        if result.trim().is_empty() || result.trim() == "{}" {
            return Ok(());
        }

        // Parse the TCL list of {channel message} pairs and send responses
        let responses = self.parse_timer_list(&result);

        for (channel, message) in responses {
            debug!("Trigger response for {} on {}: {}", channel, network, message);
            self.send_to_network(network, PluginCommand::SendToIrc {
                network: network.to_string(),
                channel,
                text: message,
            }).await?;
        }

        Ok(())
    }

    /// Check for ready timers and send their messages
    async fn check_timers(&mut self) -> Result<()> {
        // Evaluate TCL to check timers (using general timer framework)
        let result = self.tcl_thread.eval_simple("timers check".to_string()).await?;

        if result.trim().is_empty() || result.trim() == "{}" {
            return Ok(());
        }

        // Parse the TCL list of {channel message} pairs
        let timers = self.parse_timer_list(&result);

        for (channel, message) in timers {
            debug!("Timer fired for {}: {}", channel, message);
            // Timer responses: try to figure out network from channel
            // Format could be "network:#channel" or just "#channel"
            let (net, chan) = if let Some(colon_pos) = channel.find(':') {
                let n = &channel[..colon_pos];
                let c = &channel[colon_pos + 1..];
                (n.to_string(), c.to_string())
            } else {
                // Default to first network
                let default_net = self.response_channels.keys().next()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                (default_net, channel)
            };
            self.send_to_network(&net, PluginCommand::SendToIrc {
                network: net.clone(),
                channel: chan,
                text: message,
            }).await?;
        }

        Ok(())
    }

    /// Parse a TCL list of {channel message} pairs
    fn parse_timer_list(&self, tcl_list: &str) -> Vec<(String, String)> {
        let mut result = Vec::new();
        let trimmed = tcl_list.trim();

        if trimmed.is_empty() {
            return result;
        }

        // Simple parser for TCL list format
        // Each element is {channel message}
        let mut depth = 0;
        let mut current = String::new();
        let mut in_element = false;

        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        in_element = true;
                        current.clear();
                    } else {
                        current.push(ch);
                    }
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 && in_element {
                        // Parse {channel message}
                        if let Some((channel, message)) = self.parse_timer_element(&current) {
                            result.push((channel, message));
                        }
                        in_element = false;
                    } else if depth > 0 {
                        current.push(ch);
                    }
                }
                _ => {
                    if in_element {
                        current.push(ch);
                    }
                }
            }
        }

        result
    }

    /// Parse a single timer element: "{channel} {message}" or "channel message"
    fn parse_timer_element(&self, element: &str) -> Option<(String, String)> {
        let trimmed = element.trim();

        // Check if message is braced
        if let Some(space_idx) = trimmed.find(' ') {
            let channel_part = trimmed[..space_idx].to_string();
            let rest = trimmed[space_idx + 1..].trim();

            // Handle braced channel (TCL list format)
            let channel = if channel_part.starts_with('{') && channel_part.ends_with('}') {
                channel_part[1..channel_part.len() - 1].to_string()
            } else {
                channel_part
            };

            // Handle braced message
            let message = if rest.starts_with('{') && rest.ends_with('}') {
                rest[1..rest.len() - 1].to_string()
            } else {
                rest.to_string()
            };

            return Some((channel, message));
        }

        None
    }

    async fn handle_eval(
        &mut self,
        message: Message,
        is_admin: bool,
    ) -> Result<()> {
        let network = message.author.network.clone();

        // Clean up old cache entries (older than 5 minutes)
        self.cleanup_cache();

        // Extract the command (remove "tcl " or "tclAdmin " prefix)
        let code = if message.content.starts_with("tclAdmin ") {
            message.content.strip_prefix("tclAdmin ").unwrap_or(&message.content)
        } else if message.content.starts_with("tcl ") {
            message.content.strip_prefix("tcl ").unwrap_or(&message.content)
        } else {
            &message.content
        };

        // Handle "more" command to retrieve cached output
        if code.trim() == "more" {
            return self.handle_more_command(&message, &network).await;
        }

        // Handle admin blacklist commands
        if code.trim() == "blacklist" || code.trim().starts_with("blacklist ") {
            return self.handle_blacklist_command(&message, is_admin, code.trim(), &network).await;
        }

        // Validate bracket balancing
        if let Err(e) = validator::validate_brackets(code) {
            self.send_response(&message, format!("error: {}", e), &network)
                .await?;
            return Ok(());
        }

        debug!("Evaluating TCL: {} (admin={})", code, is_admin);

        // Build hostmask for privilege and blacklist checking: nick!ident@host
        let ident = message.author.ident.clone().unwrap_or_else(|| "user".to_string());
        let host_part = message.author.host.clone().unwrap_or_else(|| "irc".to_string());
        let full_host = format!("{}@{}", ident, host_part);
        let user_hostmask = format!("{}!{}", message.author.nick, full_host);

        // Check if user is blacklisted
        let blacklisted_pattern = self.security_config.blacklisted_users.iter()
            .find(|pattern| crate::hostmask::matches_hostmask(&user_hostmask, pattern))
            .cloned();

        if let Some(pattern) = blacklisted_pattern {
            let msg = "error: you are blacklisted and cannot use this bot";
            self.send_response(&message, msg.to_string(), &network).await?;
            info!("Blocked blacklisted user: {} (matched pattern: {})", user_hostmask, pattern);
            return Ok(());
        }

        // Send to TCL thread with timeout
        let result = self.tcl_thread.eval(
            code.to_string(),
            is_admin,
            message.author.nick.clone(),
            full_host,
            message.author.channel.clone(),
        ).await?;

        debug!("TCL eval completed, output length: {} bytes", result.output.len());

        // Send PM notifications to admins if state was committed
        if let Some(ref commit_info) = result.commit_info {
            debug!("Sending commit notifications");
            self.send_commit_notifications(commit_info, &message, &network).await?;
        }

        debug!("Starting response send with timeout");
        // Send response with same timeout as TCL evaluation to prevent hanging on huge output
        let timeout = Duration::from_millis(self.security_config.eval_timeout_ms);
        match tokio::time::timeout(
            timeout,
            self.send_response(&message, result.output, &network)
        ).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                warn!("Response sending timed out after {}ms, likely huge output", self.security_config.eval_timeout_ms);
                // Try to send error message
                let _ = self.send_to_network(&network, PluginCommand::SendToIrc {
                    network: network.clone(),
                    channel: message.author.channel.clone(),
                    text: "error: output too large, response timed out".to_string(),
                }).await;
                Ok(())
            }
        }
    }

    async fn send_response(
        &mut self,
        original_message: &Message,
        output: String,
        network: &str,
    ) -> Result<()> {
        debug!("send_response called with {} bytes", output.len());

        // Reject output that's too large
        const MAX_OUTPUT_BYTES: usize = 100_000; // 100KB max
        if output.len() > MAX_OUTPUT_BYTES {
            warn!("Output too large ({} bytes), sending error instead", output.len());
            self.send_to_network(network, PluginCommand::SendToIrc {
                network: network.to_string(),
                channel: original_message.author.channel.clone(),
                text: format!("error: output too large ({} bytes, max {} bytes)",
                             output.len(), MAX_OUTPUT_BYTES),
            }).await?;
            return Ok(());
        }

        debug!("About to split {} bytes into lines", output.len());

        // Split output into lines
        let all_lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        debug!("Split into {} lines", all_lines.len());

        let max_lines = self.tcl_config.max_output_lines;

        let (output, cache_remaining) = if all_lines.len() > max_lines {
            // Store remaining lines in cache
            let cache_key = (
                original_message.author.channel.clone(),
                original_message.author.nick.clone(),
            );

            let total_lines = all_lines.len();
            let shown_lines: Vec<String> = all_lines.iter().take(max_lines).cloned().collect();

            self.output_cache.insert(
                cache_key,
                OutputCache {
                    lines: all_lines,
                    offset: max_lines,
                    timestamp: Instant::now(),
                },
            );

            (
                format!(
                    "{}\n... ({} more lines - type 'tcl more' to continue)",
                    shown_lines.join("\n"),
                    total_lines - max_lines
                ),
                true,
            )
        } else {
            (output, false)
        };

        self.send_to_network(network, PluginCommand::SendToIrc {
            network: network.to_string(),
            channel: original_message.author.channel.clone(),
            text: output,
        }).await?;

        // Clean up cache entry if we showed all lines
        if !cache_remaining {
            let cache_key = (
                original_message.author.channel.clone(),
                original_message.author.nick.clone(),
            );
            self.output_cache.remove(&cache_key);
        }

        Ok(())
    }

    /// Send private message notifications to admins about git commits
    async fn send_commit_notifications(
        &self,
        commit_info: &crate::state::CommitInfo,
        original_message: &Message,
        network: &str,
    ) -> Result<()> {
        // Build notification message
        let notification = format!(
            "[Git] {} by {} | {}",
            &commit_info.commit_id[..8],
            commit_info.author,
            commit_info.changes_summary
        );

        // Send PM to each online admin (tracked via join/part/quit events)
        for admin_nick in &self.admin_nicks {
            let is_sender = admin_nick == &original_message.author.nick;
            if !is_sender || self.security_config.notify_self {
                debug!("Sending commit notification to {}", admin_nick);
                self.send_to_network(network, PluginCommand::SendToIrc {
                    network: network.to_string(),
                    channel: admin_nick.clone(), // In IRC, nick as channel = PM
                    text: notification.clone(),
                }).await?;
            }
        }

        Ok(())
    }

    /// Check if a user is an admin and update the admin_nicks set
    fn update_admin_status(&mut self, nick: &str, mask: &str, add: bool) {
        // Build full hostmask: nick!ident@host
        let hostmask = format!("{}!{}", nick, mask);

        // Check if hostmask matches any privileged pattern
        let is_admin = self.security_config.privileged_users.iter().any(|pattern| {
            hostmask::matches_hostmask(&hostmask, pattern)
        });

        if is_admin {
            if add {
                self.admin_nicks.insert(nick.to_string());
                debug!("Added {} to admin nicks (matched privileged pattern)", nick);
            }
        } else if !add {
            self.admin_nicks.remove(nick);
        }
    }

    /// Clean up cache entries older than 5 minutes
    fn cleanup_cache(&mut self) {
        let now = Instant::now();
        let timeout = Duration::from_secs(300); // 5 minutes

        self.output_cache.retain(|_, cache| {
            now.duration_since(cache.timestamp) < timeout
        });
    }

    /// Handle "more" command to show next chunk of cached output
    async fn handle_more_command(
        &mut self,
        message: &Message,
        network: &str,
    ) -> Result<()> {
        let cache_key = (
            message.author.channel.clone(),
            message.author.nick.clone(),
        );

        if let Some(cache) = self.output_cache.get_mut(&cache_key) {
            let max_lines = self.tcl_config.max_output_lines;
            let remaining = cache.lines.len() - cache.offset;

            if remaining == 0 {
                // No more lines
                self.send_to_network(network, PluginCommand::SendToIrc {
                    network: network.to_string(),
                    channel: message.author.channel.clone(),
                    text: "No more output.".to_string(),
                }).await?;
                self.output_cache.remove(&cache_key);
                return Ok(());
            }

            // Get next chunk of lines
            let end = std::cmp::min(cache.offset + max_lines, cache.lines.len());
            let chunk: Vec<String> = cache.lines[cache.offset..end].to_vec();
            let new_offset = end;
            let still_remaining = cache.lines.len() - new_offset;

            // Update offset
            cache.offset = new_offset;

            // Build output
            let output = if still_remaining > 0 {
                format!(
                    "{}\n... ({} more lines - type 'tcl more' to continue)",
                    chunk.join("\n"),
                    still_remaining
                )
            } else {
                chunk.join("\n")
            };

            self.send_to_network(network, PluginCommand::SendToIrc {
                network: network.to_string(),
                channel: message.author.channel.clone(),
                text: output,
            }).await?;

            // Clean up if we showed all lines
            if still_remaining == 0 {
                self.output_cache.remove(&cache_key);
            }
        } else {
            self.send_to_network(network, PluginCommand::SendToIrc {
                network: network.to_string(),
                channel: message.author.channel.clone(),
                text: "No cached output. Run a tcl command first.".to_string(),
            }).await?;
        }

        Ok(())
    }

    /// Handle admin "blacklist" commands
    async fn handle_blacklist_command(
        &mut self,
        message: &Message,
        is_admin: bool,
        code: &str,
        network: &str,
    ) -> Result<()> {
        // Blacklist commands are admin-only
        if !is_admin {
            self.send_response(message, "error: blacklist commands require admin privileges (use tclAdmin)".to_string(), network).await?;
            return Ok(());
        }

        let parts: Vec<&str> = code.split_whitespace().collect();

        if parts.len() < 2 {
            self.send_response(message, "error: usage: blacklist <add|remove|list> [hostmask]".to_string(), network).await?;
            return Ok(());
        }

        let subcommand = parts[1];

        match subcommand {
            "add" => {
                if parts.len() < 3 {
                    self.send_response(message, "error: usage: blacklist add <hostmask>".to_string(), network).await?;
                    return Ok(());
                }

                let hostmask = parts[2..].join(" ");

                // Check if already blacklisted
                if self.security_config.blacklisted_users.contains(&hostmask) {
                    self.send_response(message, format!("Hostmask '{}' is already blacklisted", hostmask), network).await?;
                    return Ok(());
                }

                // Add to blacklist
                self.security_config.blacklisted_users.push(hostmask.clone());
                info!("Admin {} added '{}' to blacklist", message.author.nick, hostmask);
                self.send_response(message, format!("Added '{}' to blacklist (runtime only, not saved to config)", hostmask), network).await?;
            }

            "remove" => {
                if parts.len() < 3 {
                    self.send_response(message, "error: usage: blacklist remove <hostmask>".to_string(), network).await?;
                    return Ok(());
                }

                let hostmask = parts[2..].join(" ");

                // Find and remove
                if let Some(pos) = self.security_config.blacklisted_users.iter().position(|x| x == &hostmask) {
                    self.security_config.blacklisted_users.remove(pos);
                    info!("Admin {} removed '{}' from blacklist", message.author.nick, hostmask);
                    self.send_response(message, format!("Removed '{}' from blacklist", hostmask), network).await?;
                } else {
                    self.send_response(message, format!("Hostmask '{}' is not in blacklist", hostmask), network).await?;
                }
            }

            "list" => {
                if self.security_config.blacklisted_users.is_empty() {
                    self.send_response(message, "Blacklist is empty".to_string(), network).await?;
                } else {
                    let list = self.security_config.blacklisted_users.join(", ");
                    self.send_response(message, format!("Blacklisted hostmasks ({}): {}", self.security_config.blacklisted_users.len(), list), network).await?;
                }
            }

            _ => {
                self.send_response(message, format!("error: unknown blacklist subcommand '{}'. Use: add, remove, or list", subcommand), network).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a minimal TclPlugin for testing parse functions
    fn create_test_plugin() -> TclPlugin {
        use crate::config::{SecurityConfig, ServerConfig, TclConfig};
        use std::collections::HashMap;
        use std::sync::{Arc, RwLock};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state");
        let config_path = temp_dir.path().join("config.toml");

        let security_config = SecurityConfig {
            eval_timeout_ms: 5000,
            memory_limit_mb: 0,
            max_recursion_depth: 1000,
            privileged_users: vec![],
            blacklisted_users: vec![],
            notify_self: false,
        };

        let tcl_config = TclConfig {
            state_path,
            state_repo: None,
            ssh_key: None,
            max_output_lines: 10,
            show_error_traces: false,
        };

        let server_config = ServerConfig {
            name: None,
            hostname: "irc.example.com".to_string(),
            port: 6667,
            use_tls: false,
            nickname: "testbot".to_string(),
            channels: vec!["#test".to_string()],
        };

        let channel_members: ChannelMembers = Arc::new(RwLock::new(HashMap::new()));

        TclPlugin::new(security_config, tcl_config, vec![server_config], config_path, channel_members).unwrap()
    }

    #[test]
    fn test_parse_timer_element_braced() {
        let plugin = create_test_plugin();

        // Test format: {channel} {message}
        let result = plugin.parse_timer_element("{#test} {Hello world}");
        assert_eq!(result, Some(("#test".to_string(), "Hello world".to_string())));
    }

    #[test]
    fn test_parse_timer_element_unbraced() {
        let plugin = create_test_plugin();

        // Test format: channel message
        let result = plugin.parse_timer_element("#test Hello");
        assert_eq!(result, Some(("#test".to_string(), "Hello".to_string())));
    }

    #[test]
    fn test_parse_timer_element_braced_message_only() {
        let plugin = create_test_plugin();

        // Test format: channel {message with spaces}
        let result = plugin.parse_timer_element("#test {Hello world}");
        assert_eq!(result, Some(("#test".to_string(), "Hello world".to_string())));
    }

    #[test]
    fn test_parse_timer_list_single() {
        let plugin = create_test_plugin();

        // Test single timer in list
        let result = plugin.parse_timer_list("{{#test} {Hello world}}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("#test".to_string(), "Hello world".to_string()));
    }

    #[test]
    fn test_parse_timer_list_multiple() {
        let plugin = create_test_plugin();

        // Test multiple timers in list
        let result = plugin.parse_timer_list("{{#test} {Hello}} {{#chan2} {World}}");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("#test".to_string(), "Hello".to_string()));
        assert_eq!(result[1], ("#chan2".to_string(), "World".to_string()));
    }

    #[test]
    fn test_parse_timer_list_empty() {
        let plugin = create_test_plugin();

        let result = plugin.parse_timer_list("");
        assert_eq!(result.len(), 0);

        let result = plugin.parse_timer_list("{}");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_timer_list_stare_message() {
        let plugin = create_test_plugin();

        // Test the actual stare message format
        let result = plugin.parse_timer_list("{{#bottest} {TIMTOM IS STARING AT WRATH}}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("#bottest".to_string(), "TIMTOM IS STARING AT WRATH".to_string()));
    }

    #[test]
    fn test_parse_trigger_response() {
        let plugin = create_test_plugin();

        // Test trigger dispatch response format (same as timer format)
        let result = plugin.parse_timer_list("{{#test} {Welcome testuser!}}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("#test".to_string(), "Welcome testuser!".to_string()));
    }
}
