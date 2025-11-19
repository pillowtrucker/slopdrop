use crate::config::ServerConfig;
use crate::irc_formatting;
use crate::types::{ChannelMembers, Message, MessageAuthor, PluginCommand};
use anyhow::Result;
use futures::StreamExt;
use irc::client::prelude::*;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct IrcClient {
    client: Client,
    /// Server configuration (kept for potential future use, e.g., reconnection)
    #[allow(dead_code)]
    config: ServerConfig,
    channel_members: ChannelMembers,
    /// Channels to join after registration
    channels_to_join: Vec<String>,
}

impl IrcClient {
    pub async fn new(config: ServerConfig, channel_members: ChannelMembers) -> Result<Self> {
        // Store channels to join after registration
        let channels_to_join = config.channels.clone();

        let irc_config = Config {
            nickname: Some(config.nickname.clone()),
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
        })
    }

    /// Main event loop for the IRC client
    pub async fn run(
        mut self,
        command_tx: mpsc::Sender<PluginCommand>,
        mut response_rx: mpsc::Receiver<PluginCommand>,
    ) -> Result<()> {
        let mut stream = self.client.stream()?;

        loop {
            tokio::select! {
                Some(message) = stream.next() => {
                    if let Err(e) = self.handle_irc_message(message?, &command_tx).await {
                        error!("Error handling IRC message: {}", e);
                    }
                }

                Some(command) = response_rx.recv() => {
                    if let Err(e) = self.handle_plugin_command(command).await {
                        error!("Error handling plugin command: {}", e);
                    }
                }

                else => break,
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
            Command::Response(Response::RPL_WELCOME, _) => {
                // 001 reply: Registration complete, now join channels
                info!("Registration complete, joining channels");
                for channel in &self.channels_to_join {
                    info!("Joining channel: {}", channel);
                    if let Err(e) = self.client.send_join(channel) {
                        error!("Failed to join {}: {}", channel, e);
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
                // Split long messages with smart word-boundary splitting
                for line in irc_formatting::split_message_smart(&text, 400) {
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
