use crate::config::ServerConfig;
use crate::irc_formatting;
use crate::types::{ChannelMembers, Message, MessageAuthor, PluginCommand};
use anyhow::Result;
use futures::StreamExt;
use irc::client::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::lookup_host;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Server limits and capabilities from ISUPPORT (005)
#[derive(Debug, Clone, Default)]
struct ServerLimits {
    /// Maximum message length (MSGLEN), if advertised by server
    msglen: Option<usize>,
    /// Maximum nickname length
    nicklen: Option<usize>,
    /// Maximum channel name length
    channellen: Option<usize>,
    /// Other ISUPPORT parameters
    params: HashMap<String, Option<String>>,
}

impl ServerLimits {
    /// Parse ISUPPORT parameters from a 005 message
    fn parse_isupport(&mut self, args: &[String]) {
        // 005 format: <nick> <params...> :are supported by this server
        // Skip the nickname (first arg) and the trailing message (last arg)
        for arg in args.iter().skip(1) {
            if arg.contains("are supported") {
                break;
            }

            if let Some((key, value)) = arg.split_once('=') {
                let key = key.to_uppercase();
                match key.as_str() {
                    "MSGLEN" => {
                        if let Ok(len) = value.parse::<usize>() {
                            self.msglen = Some(len);
                            debug!("Server MSGLEN: {}", len);
                        }
                    }
                    "NICKLEN" | "MAXNICKLEN" => {
                        if let Ok(len) = value.parse::<usize>() {
                            self.nicklen = Some(len);
                        }
                    }
                    "CHANNELLEN" => {
                        if let Ok(len) = value.parse::<usize>() {
                            self.channellen = Some(len);
                        }
                    }
                    _ => {}
                }
                self.params.insert(key, Some(value.to_string()));
            } else {
                // Parameter without value
                self.params.insert(arg.to_uppercase(), None);
            }
        }
    }
}

pub struct IrcClient {
    client: Client,
    /// Server configuration (kept for potential future use, e.g., reconnection)
    #[allow(dead_code)]
    config: ServerConfig,
    /// Network name for this connection
    network_name: String,
    channel_members: ChannelMembers,
    /// Channels to join after registration
    channels_to_join: Vec<String>,
    /// Desired nickname (what we want to use)
    desired_nickname: String,
    /// Current nickname attempt number (for generating alternatives)
    nick_attempt: u32,
    /// Whether we successfully registered and joined channels
    registered: bool,
    /// Server limits and capabilities from ISUPPORT (005)
    server_limits: ServerLimits,
    /// Bot's own hostmask (nick!ident@host)
    bot_hostmask: Option<String>,
}

impl IrcClient {
    pub async fn new(config: ServerConfig, network_name: String, channel_members: ChannelMembers) -> Result<Self> {
        // Store channels to join after registration
        let channels_to_join = config.channels.clone();
        let desired_nickname = config.nickname.clone();

        let irc_config = Config {
            nickname: Some(desired_nickname.clone()),
            server: Some(config.hostname.clone()),
            port: Some(config.port),
            use_tls: Some(config.use_tls),
            // Don't auto-join channels - we'll join after registration
            channels: vec![],
            // Accept self-signed certificates when using TLS
            // This is necessary for connecting to IRC servers with self-signed certs
            dangerously_accept_invalid_certs: Some(true),
            ..Default::default()
        };

        let client = Client::from_config(irc_config).await?;
        client.identify()?;

        info!("[{}] IRC client connected to {}:{}", network_name, config.hostname, config.port);

        Ok(Self {
            client,
            config,
            network_name,
            channel_members,
            channels_to_join,
            desired_nickname,
            nick_attempt: 0,
            registered: false,
            server_limits: ServerLimits::default(),
            bot_hostmask: None,
        })
    }

    /// Calculate maximum message length for a given channel
    ///
    /// Takes into account:
    /// - Server's advertised MSGLEN (if available) — NOTE: MSGLEN is the
    ///   TOTAL line length including prefix/command/target, so we subtract
    ///   the overhead from it, we do NOT use it raw (that was the old bug:
    ///   chunks were sized to the full 512-byte line, the server cut the
    ///   tail at ~446 bytes, and numbers were lost mid-word).
    /// - IRC protocol limit (512 bytes)
    /// - Overhead from: :nick!ident@host PRIVMSG #channel :\r\n
    /// - A safety margin so hostmask drift (cloaks applied after RPL_WELCOME,
    ///   ident changes, etc.) can never push a chunk over the server limit.
    fn calculate_max_message_length(&self, channel: &str) -> usize {
        // Extra bytes kept free so the server NEVER truncates a chunk.
        // The hostmask estimate can be stale (cloaks are applied after
        // RPL_WELCOME), and MSGLEN counts the whole line. Without this
        // margin, a chunk that is exactly at the limit gets cut mid-word
        // and the tail (numbers, words) is silently DISCARDED.
        const SAFETY_MARGIN: usize = 20;

        // Total line budget: server-advertised MSGLEN if present, else the
        // IRC protocol maximum. MSGLEN is the WHOLE line, not the content.
        let total_len = self.server_limits.msglen.unwrap_or(512);

        // Overhead: ":nick!ident@host PRIVMSG #channel :\r\n"
        let overhead = if let Some(ref hostmask) = self.bot_hostmask {
            // :nick!ident@host (1 + hostmask length)
            let prefix_len = 1 + hostmask.len();
            // " PRIVMSG " (9 bytes)
            let command_len = 9;
            // "#channel " (channel + space = channel.len() + 1)
            let target_len = channel.len() + 1;
            // ":\r\n" (3 bytes)
            let suffix_len = 3;

            prefix_len + command_len + target_len + suffix_len
        } else {
            // Conservative estimate if we don't know our hostmask yet
            // Use server-provided limits if available, otherwise use RFC1459 maximums
            let max_nick = self.server_limits.nicklen.unwrap_or(30);
            // RFC1459: username is max 10 chars, but some servers allow more
            let max_ident = 10;
            // RFC1459: hostname is max 63 chars
            let max_host = 63;

            // Assume worst case: maxnick + maxident + maxhost
            let estimated_prefix = 1 + max_nick + 1 + max_ident + 1 + max_host; // :nick!ident@host
            let command_len = 9; // " PRIVMSG "
            let target_len = channel.len() + 1; // "#channel "
            let suffix_len = 3; // ":\r\n"

            estimated_prefix + command_len + target_len + suffix_len
        };

        // Available space for message content: total line minus overhead,
        // minus the safety margin so the server never truncates mid-chunk.
        let max_len = total_len.saturating_sub(overhead + SAFETY_MARGIN);

        // Ensure we have at least some reasonable minimum (100 bytes)
        // Upper limit of 480 bytes leaves margin for edge cases while allowing
        // most of the calculated space to be used
        max_len.clamp(100, 480)
    }

    /// Generate an alternative nickname when the desired one is in use
    /// Strategies: append _ for first few attempts, then add numbers
    fn generate_alternative_nick(&self) -> String {
        match self.nick_attempt {
            0 => self.desired_nickname.clone(),
            1..=3 => format!("{}{}", self.desired_nickname, "_".repeat(self.nick_attempt as usize)),
            n => format!("{}_{}", self.desired_nickname, n - 3),
        }
    }

    /// Attempt to reclaim the desired nickname
    fn try_reclaim_nick(&mut self) -> Result<()> {
        let current = self.client.current_nickname();
        if current != self.desired_nickname && self.registered {
            info!("[{}] Attempting to reclaim desired nickname: {}", self.network_name, self.desired_nickname);
            self.client.send(Command::NICK(self.desired_nickname.clone()))?;
        }
        Ok(())
    }

    /// Composite key for channel members: "network:#channel"
    fn member_key(&self, channel: &str) -> String {
        format!("{}:{}", self.network_name, channel)
    }

    /// Main event loop for the IRC client
    pub async fn run(
        mut self,
        command_tx: mpsc::Sender<PluginCommand>,
        response_rx: &mut mpsc::Receiver<PluginCommand>,
    ) -> Result<()> {
        let mut stream = self.client.stream()?;
        info!("[{}] IRC event loop started, waiting for messages...", self.network_name);

        // Timer for periodic nickname reclaim attempts (every 5 minutes)
        let mut nick_reclaim_interval = tokio::time::interval(Duration::from_secs(300));
        nick_reclaim_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(message)) => {
                            debug!("[{}] Received IRC message: {:?}", self.network_name, message);
                            if let Err(e) = self.handle_irc_message(message, &command_tx).await {
                                error!("[{}] Error handling IRC message: {}", self.network_name, e);
                            }
                        }
                        Some(Err(e)) => {
                            error!("[{}] IRC connection error: {}", self.network_name, e);
                            info!("[{}] IRC connection lost - will exit", self.network_name);
                            break;
                        }
                        None => {
                            info!("[{}] IRC stream closed by server", self.network_name);
                            break;
                        }
                    }
                }

                Some(command) = response_rx.recv() => {
                    if let Err(e) = self.handle_plugin_command(command).await {
                        error!("[{}] Error handling plugin command: {}", self.network_name, e);
                    }
                }

                _ = nick_reclaim_interval.tick() => {
                    // Periodically try to reclaim our desired nickname
                    if let Err(e) = self.try_reclaim_nick() {
                        debug!("[{}] Failed to reclaim nickname: {}", self.network_name, e);
                    }
                }

                else => {
                    info!("[{}] IRC event loop ending - response channel closed", self.network_name);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_irc_message(
        &mut self,
        message: irc::proto::Message,
        command_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        let net = self.network_name.clone();
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    // If this is our own message echoed back by the server,
                    // refresh the hostmask from the ACTUAL prefix the server
                    // used. Cloaks are often applied after RPL_WELCOME, so
                    // the welcome-time hostmask can be shorter than the real
                    // one — the echo is ground truth. (Only update if the
                    // echo is consistent: same nick, and target is a channel
                    // or our own nick.)
                    if nick == &self.client.current_nickname() {
                        let echoed_hostmask = format!("{}!{}@{}", nick, user, host);
                        let should_update = match self.bot_hostmask {
                            Some(ref current) => current != &echoed_hostmask,
                            None => true,
                        };
                        if should_update {
                            debug!(
                                "[{}] Refreshing bot hostmask from echo: {}",
                                net, echoed_hostmask
                            );
                            self.bot_hostmask = Some(echoed_hostmask);
                        }
                    }

                    // Strip IRC formatting codes from the message
                    let clean_msg = irc_formatting::strip_irc_formatting(msg);

                    // Log all public messages to channel history and send TEXT event
                    if target.starts_with('#') {
                        let mask = format!("{}@{}", user, host);
                        command_tx
                            .send(PluginCommand::LogMessage {
                                network: net.clone(),
                                channel: target.clone(),
                                nick: nick.clone(),
                                mask: mask.clone(),
                                text: clean_msg.clone(),
                            })
                            .await?;

                        // Send TEXT event for trigger handling
                        command_tx
                            .send(PluginCommand::UserText {
                                network: net.clone(),
                                channel: target.clone(),
                                nick: nick.clone(),
                                mask,
                                text: clean_msg.clone(),
                            })
                            .await?;
                    }

                    // Check if message starts with "tcl " or "tclAdmin "
                    if clean_msg.starts_with("tcl ") || clean_msg.starts_with("tclAdmin ") {
                        // Only respond to commands in channels, not private messages
                        if !target.starts_with('#') {
                            debug!("[{}] Ignoring tcl command from private message ({})", net, nick);
                            return Ok(());
                        }

                        let is_admin = clean_msg.starts_with("tclAdmin ");
                        let channel = target.clone();

                        let author = MessageAuthor::new(nick.clone(), channel)
                            .with_ident(user.clone())
                            .with_host(host.clone())
                            .with_network(net.clone());

                        let content = clean_msg;

                        debug!("[{}] Received command from {}: {}", net, author, content);

                        command_tx
                            .send(PluginCommand::EvalTcl {
                                message: Message::new(author, content),
                                is_admin,
                            })
                            .await?;
                    }
                }
            }
            Command::INVITE(ref _nick, ref channel) => {
                debug!("[{}] Invited to {}, joining", net, channel);
                self.client.send_join(channel)?;
            }
            Command::KICK(ref channel, ref nick, ref reason) => {
                if nick == self.client.current_nickname() {
                    info!("[{}] Kicked from {}, rejoining in 10s", net, channel);
                    // Wait 10 seconds then automatically rejoin
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    let _ = self.client.send_join(channel);
                } else {
                    // Someone else was kicked, remove from member list
                    self.remove_member(channel, nick);

                    // Send event to plugin for trigger handling
                    let kicker = if let Some(Prefix::Nickname(ref kicker_nick, _, _)) = message.prefix {
                        kicker_nick.clone()
                    } else {
                        "unknown".to_string()
                    };
                    command_tx
                        .send(PluginCommand::UserKick {
                            network: net.clone(),
                            channel: channel.clone(),
                            nick: nick.clone(),
                            kicker,
                            reason: reason.clone().unwrap_or_default(),
                        })
                        .await?;
                }
            }
            Command::JOIN(ref channel, _, _) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    debug!("[{}] {} joined {}", net, nick, channel);
                    self.add_member(channel, nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserJoin {
                            network: net.clone(),
                            channel: channel.clone(),
                            nick: nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::PART(ref channel, _) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    debug!("[{}] {} left {}", net, nick, channel);
                    self.remove_member(channel, nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserPart {
                            network: net.clone(),
                            channel: channel.clone(),
                            nick: nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::QUIT(ref quit_msg) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    debug!("[{}] {} quit", net, nick);
                    self.remove_member_from_all(nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserQuit {
                            network: net.clone(),
                            nick: nick.clone(),
                            mask,
                            message: quit_msg.clone().unwrap_or_default(),
                        })
                        .await?;
                }
            }
            Command::NICK(ref new_nick) => {
                if let Some(Prefix::Nickname(ref old_nick, ref user, ref host)) = message.prefix {
                    debug!("[{}] {} changed nick to {}", net, old_nick, new_nick);

                    // Check if this is our own nick change
                    if old_nick == self.client.current_nickname() {
                        if new_nick == &self.desired_nickname {
                            info!("[{}] Successfully reclaimed desired nickname: {}", net, self.desired_nickname);
                            self.nick_attempt = 0;
                        } else {
                            debug!("[{}] Our nickname changed to: {}", net, new_nick);
                        }
                    }

                    self.rename_member(old_nick, new_nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserNick {
                            network: net.clone(),
                            old_nick: old_nick.clone(),
                            new_nick: new_nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::Response(Response::RPL_NAMREPLY, ref args) => {
                if args.len() >= 4 {
                    let channel = &args[2];
                    let nicks_str = &args[3];

                    debug!("[{}] NAMES for {}: {}", net, channel, nicks_str);

                    for nick in nicks_str.split_whitespace() {
                        // Strip mode prefixes (@, +, etc.)
                        let clean_nick = nick.trim_start_matches(|c| c == '@' || c == '+' || c == '%' || c == '&' || c == '~');
                        self.add_member(channel, clean_nick);
                    }
                }
            }
            Command::Response(Response::RPL_ENDOFNAMES, _) => {
                debug!("[{}] End of NAMES list", net);
            }
            Command::Response(Response::RPL_ISUPPORT, ref args) => {
                debug!("[{}] Received ISUPPORT: {:?}", net, args);
                self.server_limits.parse_isupport(args);
            }
            Command::Response(Response::RPL_WELCOME, ref args) => {
                self.registered = true;
                let current_nick = self.client.current_nickname();

                if let Some(welcome_msg) = args.last() {
                    if let Some(hostmask_start) = welcome_msg.rfind(char::is_whitespace) {
                        let potential_hostmask = &welcome_msg[hostmask_start + 1..];
                        if potential_hostmask.contains('!') && potential_hostmask.contains('@') {
                            self.bot_hostmask = Some(potential_hostmask.to_string());
                            info!("[{}] Bot hostmask: {}", net, potential_hostmask);
                        }
                    }
                }

                if self.bot_hostmask.is_none() {
                    debug!("[{}] Requesting hostmask via USERHOST", net);
                    let _ = self.client.send(Command::USERHOST(vec![current_nick.to_string()]));
                }

                if current_nick != self.desired_nickname {
                    warn!("[{}] Registered with alternative nickname: {} (desired: {})",
                          net, current_nick, self.desired_nickname);
                    warn!("[{}] Will attempt to reclaim {} periodically", net, self.desired_nickname);
                } else {
                    info!("[{}] Registration complete with desired nickname: {}", net, current_nick);
                }

                info!("[{}] Joining channels", net);
                for channel in &self.channels_to_join {
                    info!("[{}] Joining channel: {}", net, channel);
                    if let Err(e) = self.client.send_join(channel) {
                        error!("[{}] Failed to join {}: {}", net, channel, e);
                    }
                }
            }
            Command::Response(Response::RPL_USERHOST, ref args) => {
                if let Some(response) = args.get(1) {
                    debug!("[{}] USERHOST response: {}", net, response);
                    if let Some(eq_pos) = response.find('=') {
                        let nick_part = &response[..eq_pos].trim_end_matches('*');
                        let rest = &response[eq_pos + 1..].trim_start_matches(&['+', '-'][..]);
                        if let Some(at_pos) = rest.find('@') {
                            let ident = &rest[..at_pos];
                            let host = &rest[at_pos + 1..];
                            let hostmask = format!("{}!{}@{}", nick_part, ident, host);
                            self.bot_hostmask = Some(hostmask.clone());
                            info!("[{}] Bot hostmask from USERHOST: {}", net, hostmask);
                        }
                    }
                }
            }
            Command::Response(Response::ERR_NICKNAMEINUSE, _) => {
                self.nick_attempt += 1;
                let alt_nick = self.generate_alternative_nick();

                if self.registered {
                    debug!("[{}] Nickname {} still in use, cannot reclaim yet", net, self.desired_nickname);
                } else {
                    warn!("[{}] Nickname {} is in use, trying alternative: {}",
                          net, self.desired_nickname, alt_nick);

                    if let Err(e) = self.client.send(Command::NICK(alt_nick)) {
                        error!("[{}] Failed to send NICK command: {}", net, e);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_plugin_command(&self, command: PluginCommand) -> Result<()> {
        match command {
            PluginCommand::SendToIrc { channel, text, .. } => {
                let max_len = self.calculate_max_message_length(&channel);
                debug!("[{}] Using max message length {} for channel {}", self.network_name, max_len, channel);

                for line in irc_formatting::split_message_smart(&text, max_len) {
                    self.client.send_privmsg(&channel, &line)?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
            PluginCommand::Shutdown => {
                info!("[{}] Shutting down IRC client", self.network_name);
                self.client.send_quit("Goodbye")?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Add a member to a channel (using composite network:channel key)
    fn add_member(&self, channel: &str, nick: &str) {
        use std::collections::HashSet;
        let key = self.member_key(channel);
        let mut members = self.channel_members.write().unwrap();
        members
            .entry(key)
            .or_insert_with(HashSet::new)
            .insert(nick.to_string());
    }

    /// Remove a member from a channel (using composite key)
    fn remove_member(&self, channel: &str, nick: &str) {
        let key = self.member_key(channel);
        let mut members = self.channel_members.write().unwrap();
        if let Some(channel_set) = members.get_mut(&key) {
            channel_set.remove(nick);
        }
    }

    /// Remove a member from all channels on this network (for QUIT)
    fn remove_member_from_all(&self, nick: &str) {
        let prefix = format!("{}:", self.network_name);
        let mut members = self.channel_members.write().unwrap();
        for (key, channel_set) in members.iter_mut() {
            if key.starts_with(&prefix) {
                channel_set.remove(nick);
            }
        }
    }

    /// Rename a member in all channels on this network (for NICK)
    fn rename_member(&self, old_nick: &str, new_nick: &str) {
        let prefix = format!("{}:", self.network_name);
        let mut members = self.channel_members.write().unwrap();
        for (key, channel_set) in members.iter_mut() {
            if key.starts_with(&prefix) {
                if channel_set.remove(old_nick) {
                    channel_set.insert(new_nick.to_string());
                }
            }
        }
    }
}

/// Run IRC client with automatic reconnection on failure
/// Uses exponential backoff only on connection failures.
/// On successful connection followed by disconnect, uses initial delay.
/// Cycles through DNS-resolved IPs on each reconnection attempt.
pub async fn run_with_reconnect(
    config: ServerConfig,
    network_name: String,
    channel_members: ChannelMembers,
    command_tx: mpsc::Sender<PluginCommand>,
    mut response_rx: mpsc::Receiver<PluginCommand>,
) -> Result<()> {
    const INITIAL_DELAY: u64 = 1;
    const MAX_DELAY: u64 = 300; // 5 minutes

    let mut delay_secs = INITIAL_DELAY;
    let mut server_index = 0;

    loop {
        // Resolve DNS to get all IPs for the hostname
        let lookup_addr = format!("{}:{}", config.hostname, config.port);
        let resolved_ips: Vec<_> = match lookup_host(&lookup_addr).await {
            Ok(addrs) => addrs.collect(),
            Err(e) => {
                error!("[{}] DNS lookup failed for {}: {}", network_name, config.hostname, e);
                info!("[{}] Reconnecting in {} seconds...", network_name, delay_secs);
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                delay_secs = (delay_secs * 2).min(MAX_DELAY);
                continue;
            }
        };

        if resolved_ips.is_empty() {
            error!("[{}] No IPs resolved for {}", network_name, config.hostname);
            info!("[{}] Reconnecting in {} seconds...", network_name, delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            delay_secs = (delay_secs * 2).min(MAX_DELAY);
            continue;
        }

        // Cycle through resolved IPs
        let addr = &resolved_ips[server_index % resolved_ips.len()];
        server_index += 1;

        info!("[{}] Connecting to IRC server {} ({}) [{}/{}]",
              network_name, config.hostname, addr.ip(),
              (server_index - 1) % resolved_ips.len() + 1,
              resolved_ips.len());

        // Create a modified config with the specific IP
        let mut connect_config = config.clone();
        connect_config.hostname = addr.ip().to_string();

        match IrcClient::new(connect_config, network_name.clone(), channel_members.clone()).await {
            Ok(irc_client) => {
                // Connection succeeded - reset backoff for future connection failures
                delay_secs = INITIAL_DELAY;

                // Run the client - this blocks until disconnection
                if let Err(e) = irc_client.run(command_tx.clone(), &mut response_rx).await {
                    error!("[{}] IRC client error: {}", network_name, e);
                }

                // After a successful connection + run, always use initial delay (no exponential backoff)
                info!("[{}] IRC connection lost, reconnecting in {} seconds", network_name, INITIAL_DELAY);
                tokio::time::sleep(Duration::from_secs(INITIAL_DELAY)).await;
            }
            Err(e) => {
                // Connection failed - use exponential backoff
                error!("[{}] Failed to connect to IRC: {}", network_name, e);
                info!("[{}] Reconnecting in {} seconds...", network_name, delay_secs);
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                delay_secs = (delay_secs * 2).min(MAX_DELAY);
            }
        }
    }
}
