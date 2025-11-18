use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, RwLock};

/// Shared state for tracking channel members
/// Key: channel name, Value: set of nicknames
pub type ChannelMembers = Arc<RwLock<HashMap<String, HashSet<String>>>>;

/// Represents the author/source of a message
#[derive(Debug, Clone)]
pub struct MessageAuthor {
    pub nick: String,
    pub ident: Option<String>,
    pub host: Option<String>,
    pub channel: String,
}

impl MessageAuthor {
    pub fn new(nick: String, channel: String) -> Self {
        Self {
            nick,
            ident: None,
            host: None,
            channel,
        }
    }

    pub fn with_host(mut self, host: String) -> Self {
        self.host = Some(host);
        self
    }

    pub fn with_ident(mut self, ident: String) -> Self {
        self.ident = Some(ident);
        self
    }

    /// Get full hostmask in format: nick!ident@host
    /// Returns just nick if ident/host are not available
    /// NOTE: Currently unused but part of public API for hostmask operations
    #[allow(dead_code)]
    pub fn hostmask(&self) -> String {
        match (&self.ident, &self.host) {
            (Some(ident), Some(host)) => format!("{}!{}@{}", self.nick, ident, host),
            (None, Some(host)) => format!("{}!@{}", self.nick, host),
            (Some(ident), None) => format!("{}!{}", self.nick, ident),
            (None, None) => self.nick.clone(),
        }
    }
}

impl fmt::Display for MessageAuthor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref host) = self.host {
            write!(f, "{} <{}> on {}", self.nick, host, self.channel)
        } else {
            write!(f, "{} on {}", self.nick, self.channel)
        }
    }
}

/// Represents a message flowing through the system
#[derive(Debug, Clone)]
pub struct Message {
    pub author: MessageAuthor,
    pub content: String,
}

impl Message {
    pub fn new(author: MessageAuthor, content: String) -> Self {
        Self { author, content }
    }
}

/// Commands that can be sent to plugins
#[derive(Debug, Clone)]
pub enum PluginCommand {
    /// Evaluate TCL code
    EvalTcl { message: Message, is_admin: bool },

    /// Send a message to IRC
    SendToIrc { channel: String, text: String },

    /// Log a message (for channel history)
    LogMessage {
        channel: String,
        nick: String,
        mask: String,
        text: String,
    },

    /// User joined a channel
    UserJoin {
        channel: String,
        nick: String,
        mask: String,
    },

    /// User left a channel
    UserPart {
        channel: String,
        nick: String,
        mask: String,
    },

    /// User quit IRC
    UserQuit {
        nick: String,
        mask: String,
        message: String,
    },

    /// User was kicked from a channel
    UserKick {
        channel: String,
        nick: String,
        kicker: String,
        reason: String,
    },

    /// User changed nickname
    UserNick {
        old_nick: String,
        new_nick: String,
        mask: String,
    },

    /// Shutdown the plugin
    /// NOTE: Currently unused - bot shutdown is handled differently.
    /// Kept for potential graceful shutdown implementation.
    #[allow(dead_code)]
    Shutdown,
}

/// Responses from plugins
/// NOTE: Currently unused - we use oneshot channels for responses instead.
/// Kept for potential future use.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PluginResponse {
    /// Result of TCL evaluation
    TclResult { original_message: Message, output: String },

    /// Error occurred
    Error { message: String },
}
