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
    pub async fn new(config: ServerConfig, channel_members: ChannelMembers) -> Result<Self> {
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

        info!("IRC client connected to {}:{}", config.hostname, config.port);

        Ok(Self {
            client,
            config,
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
    /// - Server's advertised MSGLEN (if available)
    /// - IRC protocol limit (512 bytes)
    /// - Overhead from: :nick!ident@host PRIVMSG #channel :\r\n
    fn calculate_max_message_length(&self, channel: &str) -> usize {
        // If server advertises MSGLEN, use that
        if let Some(msglen) = self.server_limits.msglen {
            return msglen;
        }

        // Otherwise calculate based on IRC protocol limit (512 bytes total)
        const IRC_PROTOCOL_MAX: usize = 512;

        // Calculate overhead: ":nick!ident@host PRIVMSG #channel :\r\n"
        // Format: :<prefix> PRIVMSG <target> :<trailing>\r\n
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
            // Assume worst case: 30-char nick + 10-char ident + 63-char host
            let estimated_prefix = 1 + 30 + 1 + 10 + 1 + 63; // :nick!ident@host
            let command_len = 9; // " PRIVMSG "
            let target_len = channel.len() + 1; // "#channel "
            let suffix_len = 3; // ":\r\n"

            estimated_prefix + command_len + target_len + suffix_len
        };

        // Available space for message content
        let max_len = IRC_PROTOCOL_MAX.saturating_sub(overhead);

        // Ensure we have at least some reasonable minimum (100 bytes)
        // and don't exceed a reasonable maximum (400 bytes as safety margin)
        max_len.clamp(100, 400)
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
            info!("Attempting to reclaim desired nickname: {}", self.desired_nickname);
            self.client.send(Command::NICK(self.desired_nickname.clone()))?;
        }
        Ok(())
    }

    /// Main event loop for the IRC client
    pub async fn run(
        mut self,
        command_tx: mpsc::Sender<PluginCommand>,
        response_rx: &mut mpsc::Receiver<PluginCommand>,
    ) -> Result<()> {
        let mut stream = self.client.stream()?;
        info!("IRC event loop started, waiting for messages...");

        // Timer for periodic nickname reclaim attempts (every 5 minutes)
        let mut nick_reclaim_interval = tokio::time::interval(Duration::from_secs(300));
        nick_reclaim_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(message)) => {
                            debug!("Received IRC message: {:?}", message);
                            if let Err(e) = self.handle_irc_message(message, &command_tx).await {
                                error!("Error handling IRC message: {}", e);
                            }
                        }
                        Some(Err(e)) => {
                            error!("IRC connection error: {}", e);
                            info!("IRC connection lost - will exit");
                            break;
                        }
                        None => {
                            info!("IRC stream closed by server");
                            break;
                        }
                    }
                }

                Some(command) = response_rx.recv() => {
                    if let Err(e) = self.handle_plugin_command(command).await {
                        error!("Error handling plugin command: {}", e);
                    }
                }

                _ = nick_reclaim_interval.tick() => {
                    // Periodically try to reclaim our desired nickname
                    if let Err(e) = self.try_reclaim_nick() {
                        debug!("Failed to reclaim nickname: {}", e);
                    }
                }

                else => {
                    info!("IRC event loop ending - response channel closed");
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
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    // Strip IRC formatting codes from the message
                    let clean_msg = irc_formatting::strip_irc_formatting(msg);

                    // Log all public messages to channel history and send TEXT event
                    if target.starts_with('#') {
                        let mask = format!("{}@{}", user, host);
                        command_tx
                            .send(PluginCommand::LogMessage {
                                channel: target.clone(),
                                nick: nick.clone(),
                                mask: mask.clone(),
                                text: clean_msg.clone(),
                            })
                            .await?;

                        // Send TEXT event for trigger handling
                        command_tx
                            .send(PluginCommand::UserText {
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
                            debug!("Ignoring tcl command from private message ({})", nick);
                            return Ok(());
                        }

                        let is_admin = clean_msg.starts_with("tclAdmin ");
                        let channel = target.clone();

                        let author = MessageAuthor::new(nick.clone(), channel)
                            .with_ident(user.clone())
                            .with_host(host.clone());

                        let content = clean_msg;

                        debug!("Received command from {}: {}", author, content);

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
                debug!("Invited to {}, joining", channel);
                self.client.send_join(channel)?;
            }
            Command::KICK(ref channel, ref nick, ref reason) => {
                if nick == self.client.current_nickname() {
                    info!("Kicked from {}, rejoining in 10s", channel);
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
                    debug!("{} joined {}", nick, channel);
                    self.add_member(channel, nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserJoin {
                            channel: channel.clone(),
                            nick: nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::PART(ref channel, _) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    debug!("{} left {}", nick, channel);
                    self.remove_member(channel, nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserPart {
                            channel: channel.clone(),
                            nick: nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::QUIT(ref quit_msg) => {
                if let Some(Prefix::Nickname(ref nick, ref user, ref host)) = message.prefix {
                    debug!("{} quit", nick);
                    self.remove_member_from_all(nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserQuit {
                            nick: nick.clone(),
                            mask,
                            message: quit_msg.clone().unwrap_or_default(),
                        })
                        .await?;
                }
            }
            Command::NICK(ref new_nick) => {
                if let Some(Prefix::Nickname(ref old_nick, ref user, ref host)) = message.prefix {
                    debug!("{} changed nick to {}", old_nick, new_nick);

                    // Check if this is our own nick change
                    if old_nick == self.client.current_nickname() {
                        if new_nick == &self.desired_nickname {
                            info!("Successfully reclaimed desired nickname: {}", self.desired_nickname);
                            self.nick_attempt = 0;
                        } else {
                            debug!("Our nickname changed to: {}", new_nick);
                        }
                    }

                    self.rename_member(old_nick, new_nick);

                    // Send event to plugin for trigger handling
                    let mask = format!("{}@{}", user, host);
                    command_tx
                        .send(PluginCommand::UserNick {
                            old_nick: old_nick.clone(),
                            new_nick: new_nick.clone(),
                            mask,
                        })
                        .await?;
                }
            }
            Command::Response(Response::RPL_NAMREPLY, ref args) => {
                // 353 reply: :<server> 353 <nick> <channel_type> <channel> :<nicks>
                // args[0] = our nickname
                // args[1] = channel type (=, *, @)
                // args[2] = channel name
                // args[3] = space-separated list of nicks (may have prefixes like @ or +)
                if args.len() >= 4 {
                    let channel = &args[2];
                    let nicks_str = &args[3];

                    debug!("NAMES for {}: {}", channel, nicks_str);

                    for nick in nicks_str.split_whitespace() {
                        // Strip mode prefixes (@, +, etc.)
                        let clean_nick = nick.trim_start_matches(|c| c == '@' || c == '+' || c == '%' || c == '&' || c == '~');
                        self.add_member(channel, clean_nick);
                    }
                }
            }
            Command::Response(Response::RPL_ENDOFNAMES, _) => {
                // 366 reply: :<server> 366 <nick> <channel> :End of /NAMES list.
                // This marks the end of NAMES list, we can log it
                debug!("End of NAMES list");
            }
            Command::Response(Response::RPL_ISUPPORT, ref args) => {
                // 005 reply: Server capabilities and limits
                debug!("Received ISUPPORT: {:?}", args);
                self.server_limits.parse_isupport(args);
            }
            Command::Response(Response::RPL_WELCOME, ref args) => {
                // 001 reply: Registration complete
                self.registered = true;
                let current_nick = self.client.current_nickname();

                // Try to extract hostmask from the welcome message
                // Format: ":Welcome to the Network nick!ident@host"
                if let Some(welcome_msg) = args.last() {
                    if let Some(hostmask_start) = welcome_msg.rfind(char::is_whitespace) {
                        let potential_hostmask = &welcome_msg[hostmask_start + 1..];
                        if potential_hostmask.contains('!') && potential_hostmask.contains('@') {
                            self.bot_hostmask = Some(potential_hostmask.to_string());
                            info!("Bot hostmask: {}", potential_hostmask);
                        }
                    }
                }

                // If we didn't get it from welcome message, request it via USERHOST
                if self.bot_hostmask.is_none() {
                    debug!("Requesting hostmask via USERHOST");
                    let _ = self.client.send(Command::USERHOST(vec![current_nick.to_string()]));
                }

                if current_nick != self.desired_nickname {
                    warn!("Registered with alternative nickname: {} (desired: {})",
                          current_nick, self.desired_nickname);
                    warn!("Will attempt to reclaim {} periodically", self.desired_nickname);
                } else {
                    info!("Registration complete with desired nickname: {}", current_nick);
                }

                info!("Joining channels");
                for channel in &self.channels_to_join {
                    info!("Joining channel: {}", channel);
                    if let Err(e) = self.client.send_join(channel) {
                        error!("Failed to join {}: {}", channel, e);
                    }
                }
            }
            Command::Response(Response::RPL_USERHOST, ref args) => {
                // 302 reply: USERHOST response
                // Format: :nick*=+ident@host (the * means IRC operator, + means not away)
                if let Some(response) = args.get(1) {
                    debug!("USERHOST response: {}", response);
                    // Parse: nick*=+ident@host or nick=+ident@host
                    if let Some(eq_pos) = response.find('=') {
                        let nick_part = &response[..eq_pos].trim_end_matches('*');
                        let rest = &response[eq_pos + 1..].trim_start_matches(&['+', '-'][..]);
                        if let Some(at_pos) = rest.find('@') {
                            let ident = &rest[..at_pos];
                            let host = &rest[at_pos + 1..];
                            let hostmask = format!("{}!{}@{}", nick_part, ident, host);
                            self.bot_hostmask = Some(hostmask.clone());
                            info!("Bot hostmask from USERHOST: {}", hostmask);
                        }
                    }
                }
            }
            Command::Response(Response::ERR_NICKNAMEINUSE, _) => {
                // 433 reply: Nickname is already in use
                self.nick_attempt += 1;
                let alt_nick = self.generate_alternative_nick();

                if self.registered {
                    // Already registered, just log that reclaim failed
                    debug!("Nickname {} still in use, cannot reclaim yet", self.desired_nickname);
                } else {
                    // During registration, try an alternative
                    warn!("Nickname {} is in use, trying alternative: {}",
                          self.desired_nickname, alt_nick);

                    if let Err(e) = self.client.send(Command::NICK(alt_nick)) {
                        error!("Failed to send NICK command: {}", e);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_plugin_command(&self, command: PluginCommand) -> Result<()> {
        match command {
            PluginCommand::SendToIrc { channel, text } => {
                // Calculate maximum message length for this channel dynamically
                // based on server limits and our hostmask
                let max_len = self.calculate_max_message_length(&channel);
                debug!("Using max message length {} for channel {}", max_len, channel);

                // Split long messages with smart word-boundary splitting
                for line in irc_formatting::split_message_smart(&text, max_len) {
                    self.client.send_privmsg(&channel, &line)?;
                    // Small delay to avoid flooding
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
            PluginCommand::Shutdown => {
                info!("Shutting down IRC client");
                self.client.send_quit("Goodbye")?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Add a member to a channel
    fn add_member(&self, channel: &str, nick: &str) {
        use std::collections::HashSet;
        let mut members = self.channel_members.write().unwrap();
        members
            .entry(channel.to_string())
            .or_insert_with(HashSet::new)
            .insert(nick.to_string());
    }

    /// Remove a member from a channel
    fn remove_member(&self, channel: &str, nick: &str) {
        let mut members = self.channel_members.write().unwrap();
        if let Some(channel_set) = members.get_mut(channel) {
            channel_set.remove(nick);
        }
    }

    /// Remove a member from all channels (for QUIT)
    fn remove_member_from_all(&self, nick: &str) {
        let mut members = self.channel_members.write().unwrap();
        for channel_set in members.values_mut() {
            channel_set.remove(nick);
        }
    }

    /// Rename a member in all channels (for NICK)
    fn rename_member(&self, old_nick: &str, new_nick: &str) {
        let mut members = self.channel_members.write().unwrap();
        for channel_set in members.values_mut() {
            if channel_set.remove(old_nick) {
                channel_set.insert(new_nick.to_string());
            }
        }
    }
}

/// Run IRC client with automatic reconnection on failure
/// Uses exponential backoff: 1s, 2s, 4s, 8s, ... up to 5 minutes max
/// Cycles through DNS-resolved IPs on each reconnection attempt
pub async fn run_with_reconnect(
    config: ServerConfig,
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
                error!("DNS lookup failed for {}: {}", config.hostname, e);
                info!("Reconnecting in {} seconds...", delay_secs);
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                delay_secs = (delay_secs * 2).min(MAX_DELAY);
                continue;
            }
        };

        if resolved_ips.is_empty() {
            error!("No IPs resolved for {}", config.hostname);
            info!("Reconnecting in {} seconds...", delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            delay_secs = (delay_secs * 2).min(MAX_DELAY);
            continue;
        }

        // Cycle through resolved IPs
        let addr = &resolved_ips[server_index % resolved_ips.len()];
        server_index += 1;

        info!("Connecting to IRC server {} ({}) [{}/{}]",
              config.hostname, addr.ip(),
              (server_index - 1) % resolved_ips.len() + 1,
              resolved_ips.len());

        // Create a modified config with the specific IP
        let mut connect_config = config.clone();
        connect_config.hostname = addr.ip().to_string();

        match IrcClient::new(connect_config, channel_members.clone()).await {
            Ok(irc_client) => {
                // Reset delay on successful connection
                delay_secs = INITIAL_DELAY;

                // Run the client - this blocks until disconnection
                if let Err(e) = irc_client.run(command_tx.clone(), &mut response_rx).await {
                    error!("IRC client error: {}", e);
                }

                info!("IRC connection lost, will reconnect");
            }
            Err(e) => {
                error!("Failed to connect to IRC: {}", e);
            }
        }

        info!("Reconnecting in {} seconds...", delay_secs);
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;

        // Exponential backoff
        delay_secs = (delay_secs * 2).min(MAX_DELAY);
    }
}
