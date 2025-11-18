use crate::config::{SecurityConfig, TclConfig};
use crate::tcl_thread::TclThreadHandle;
use crate::types::{ChannelMembers, Message, PluginCommand};
use crate::validator;
use anyhow::Result;
use std::collections::HashMap;
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
    /// Cache for paginated output: (channel, nick) -> remaining output
    output_cache: HashMap<(String, String), OutputCache>,
}

impl TclPlugin {
    pub fn new(
        security_config: SecurityConfig,
        tcl_config: TclConfig,
        channel_members: ChannelMembers,
    ) -> Result<Self> {
        let tcl_thread =
            TclThreadHandle::spawn(tcl_config.clone(), security_config.clone(), channel_members)?;

        Ok(Self {
            tcl_thread,
            tcl_config,
            security_config,
            output_cache: HashMap::new(),
        })
    }

    /// Main event loop for the TCL plugin
    pub async fn run(
        &mut self,
        mut command_rx: mpsc::Receiver<PluginCommand>,
        response_tx: mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        info!("TCL plugin started");

        // Timer polling interval (1 second)
        let mut timer_interval = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                // Handle incoming commands
                command = command_rx.recv() => {
                    match command {
                        Some(PluginCommand::EvalTcl { message, is_admin }) => {
                            if let Err(e) = self.handle_eval(message, is_admin, &response_tx).await {
                                error!("Error handling TCL eval: {}", e);
                            }
                        }
                        Some(PluginCommand::LogMessage { channel, nick, mask, text }) => {
                            self.tcl_thread.log_message(channel, nick, mask, text);
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
                    if let Err(e) = self.check_timers(&response_tx).await {
                        warn!("Error checking timers: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check for ready timers and send their messages
    async fn check_timers(&mut self, response_tx: &mpsc::Sender<PluginCommand>) -> Result<()> {
        // Evaluate TCL to check timers
        let result = self.tcl_thread.eval_simple("timtom check_timers".to_string()).await?;

        if result.trim().is_empty() || result.trim() == "{}" {
            return Ok(());
        }

        // Parse the TCL list of {channel message} pairs
        // Format: {{channel1 message1} {channel2 message2} ...}
        let timers = self.parse_timer_list(&result);

        for (channel, message) in timers {
            debug!("Timer fired for {}: {}", channel, message);
            response_tx
                .send(PluginCommand::SendToIrc {
                    channel,
                    text: message,
                })
                .await?;
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

    /// Parse a single timer element: "channel message"
    fn parse_timer_element(&self, element: &str) -> Option<(String, String)> {
        let trimmed = element.trim();

        // Check if message is braced
        if let Some(space_idx) = trimmed.find(' ') {
            let channel = trimmed[..space_idx].to_string();
            let rest = trimmed[space_idx + 1..].trim();

            // Handle braced message
            if rest.starts_with('{') && rest.ends_with('}') {
                let message = rest[1..rest.len() - 1].to_string();
                return Some((channel, message));
            } else {
                return Some((channel, rest.to_string()));
            }
        }

        None
    }

    async fn handle_eval(
        &mut self,
        message: Message,
        is_admin: bool,
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
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
            return self.handle_more_command(&message, response_tx).await;
        }

        // Handle admin blacklist commands
        if code.trim().starts_with("blacklist ") {
            return self.handle_blacklist_command(&message, is_admin, code.trim(), response_tx).await;
        }

        // Validate bracket balancing
        if let Err(e) = validator::validate_brackets(code) {
            self.send_response(&message, format!("error: {}", e), response_tx)
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
            self.send_response(&message, msg.to_string(), response_tx).await?;
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

        // Send PM notifications to admins if state was committed
        if let Some(ref commit_info) = result.commit_info {
            self.send_commit_notifications(commit_info, &message, response_tx).await?;
        }

        self.send_response(&message, result.output, response_tx).await?;

        Ok(())
    }

    async fn send_response(
        &mut self,
        original_message: &Message,
        output: String,
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        // Split output into lines
        let all_lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
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

        response_tx
            .send(PluginCommand::SendToIrc {
                channel: original_message.author.channel.clone(),
                text: output,
            })
            .await?;

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
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        // Extract admin nicks from hostmask patterns
        // Simple extraction: take nick part before '!'
        let mut admin_nicks = Vec::new();
        for pattern in &self.security_config.privileged_users {
            if let Some(nick_part) = pattern.split('!').next() {
                // Skip wildcard-only patterns
                if nick_part != "*" && !nick_part.is_empty() {
                    admin_nicks.push(nick_part.to_string());
                }
            }
        }

        // Build notification message
        let notification = format!(
            "[Git] {} committed by {} | {} files changed (+{} -{}) | {}",
            &commit_info.commit_id[..8],
            commit_info.author,
            commit_info.files_changed,
            commit_info.insertions,
            commit_info.deletions,
            commit_info.message.lines().next().unwrap_or("")
        );

        // Send PM to each admin (except the sender)
        for admin_nick in admin_nicks {
            if admin_nick != original_message.author.nick {
                debug!("Sending commit notification to {}", admin_nick);
                response_tx
                    .send(PluginCommand::SendToIrc {
                        channel: admin_nick.clone(), // In IRC, nick as channel = PM
                        text: notification.clone(),
                    })
                    .await?;
            }
        }

        Ok(())
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
        response_tx: &mpsc::Sender<PluginCommand>,
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
                response_tx
                    .send(PluginCommand::SendToIrc {
                        channel: message.author.channel.clone(),
                        text: "No more output.".to_string(),
                    })
                    .await?;
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

            response_tx
                .send(PluginCommand::SendToIrc {
                    channel: message.author.channel.clone(),
                    text: output,
                })
                .await?;

            // Clean up if we showed all lines
            if still_remaining == 0 {
                self.output_cache.remove(&cache_key);
            }
        } else {
            response_tx
                .send(PluginCommand::SendToIrc {
                    channel: message.author.channel.clone(),
                    text: "No cached output. Run a tcl command first.".to_string(),
                })
                .await?;
        }

        Ok(())
    }

    /// Handle admin "blacklist" commands
    async fn handle_blacklist_command(
        &mut self,
        message: &Message,
        is_admin: bool,
        code: &str,
        response_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        // Blacklist commands are admin-only
        if !is_admin {
            self.send_response(message, "error: blacklist commands require admin privileges (use tclAdmin)".to_string(), response_tx).await?;
            return Ok(());
        }

        let parts: Vec<&str> = code.split_whitespace().collect();

        if parts.len() < 2 {
            self.send_response(message, "error: usage: blacklist <add|remove|list> [hostmask]".to_string(), response_tx).await?;
            return Ok(());
        }

        let subcommand = parts[1];

        match subcommand {
            "add" => {
                if parts.len() < 3 {
                    self.send_response(message, "error: usage: blacklist add <hostmask>".to_string(), response_tx).await?;
                    return Ok(());
                }

                let hostmask = parts[2..].join(" ");

                // Check if already blacklisted
                if self.security_config.blacklisted_users.contains(&hostmask) {
                    self.send_response(message, format!("Hostmask '{}' is already blacklisted", hostmask), response_tx).await?;
                    return Ok(());
                }

                // Add to blacklist
                self.security_config.blacklisted_users.push(hostmask.clone());
                info!("Admin {} added '{}' to blacklist", message.author.nick, hostmask);
                self.send_response(message, format!("Added '{}' to blacklist (runtime only, not saved to config)", hostmask), response_tx).await?;
            }

            "remove" => {
                if parts.len() < 3 {
                    self.send_response(message, "error: usage: blacklist remove <hostmask>".to_string(), response_tx).await?;
                    return Ok(());
                }

                let hostmask = parts[2..].join(" ");

                // Find and remove
                if let Some(pos) = self.security_config.blacklisted_users.iter().position(|x| x == &hostmask) {
                    self.security_config.blacklisted_users.remove(pos);
                    info!("Admin {} removed '{}' from blacklist", message.author.nick, hostmask);
                    self.send_response(message, format!("Removed '{}' from blacklist", hostmask), response_tx).await?;
                } else {
                    self.send_response(message, format!("Hostmask '{}' is not in blacklist", hostmask), response_tx).await?;
                }
            }

            "list" => {
                if self.security_config.blacklisted_users.is_empty() {
                    self.send_response(message, "Blacklist is empty".to_string(), response_tx).await?;
                } else {
                    let list = self.security_config.blacklisted_users.join(", ");
                    self.send_response(message, format!("Blacklisted hostmasks ({}): {}", self.security_config.blacklisted_users.len(), list), response_tx).await?;
                }
            }

            _ => {
                self.send_response(message, format!("error: unknown blacklist subcommand '{}'. Use: add, remove, or list", subcommand), response_tx).await?;
            }
        }

        Ok(())
    }
}
