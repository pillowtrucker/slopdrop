use crate::config::ServerConfig;
use crate::types::{Message, MessageAuthor, PluginCommand};
use anyhow::Result;
use futures::StreamExt;
use irc::client::prelude::*;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct IrcClient {
    client: Client,
    config: ServerConfig,
}

impl IrcClient {
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let irc_config = Config {
            nickname: Some(config.nickname.clone()),
            server: Some(config.hostname.clone()),
            port: Some(config.port),
            use_tls: Some(config.use_tls),
            channels: config.channels.clone(),
            ..Default::default()
        };

        let client = Client::from_config(irc_config).await?;
        client.identify()?;

        info!("IRC client connected to {}:{}", config.hostname, config.port);

        Ok(Self { client, config })
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
        &self,
        message: irc::proto::Message,
        command_tx: &mpsc::Sender<PluginCommand>,
    ) -> Result<()> {
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {
                if let Some(Prefix::Nickname(ref nick, ref _user, ref host)) = message.prefix {
                    // Check if message starts with "tcl " or "tclAdmin "
                    if msg.starts_with("tcl ") || msg.starts_with("tclAdmin ") {
                        let is_admin = msg.starts_with("tclAdmin ");
                        let channel = if target.starts_with('#') {
                            target.clone()
                        } else {
                            nick.clone() // Private message
                        };

                        let author = MessageAuthor::new(nick.clone(), channel)
                            .with_host(host.clone());

                        let content = msg.clone();

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
            Command::KICK(ref channel, ref nick, ref _reason) => {
                if nick == self.client.current_nickname() {
                    info!("Kicked from {}, rejoining in 10s", channel);
                    // TODO: Implement auto-rejoin
                    // Can't clone client, need to restructure for this feature
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    let _ = self.client.send_join(channel);
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_plugin_command(&self, command: PluginCommand) -> Result<()> {
        match command {
            PluginCommand::SendToIrc { channel, text } => {
                // Split long messages
                for line in Self::split_message(&text, 400) {
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

    /// Split a message into multiple lines if it's too long
    fn split_message(text: &str, max_len: usize) -> Vec<String> {
        let mut result = Vec::new();

        for line in text.lines() {
            if line.len() <= max_len {
                result.push(line.to_string());
            } else {
                // Split long lines
                let mut start = 0;
                while start < line.len() {
                    let end = (start + max_len).min(line.len());
                    result.push(line[start..end].to_string());
                    start = end;
                }
            }
        }

        result
    }
}
