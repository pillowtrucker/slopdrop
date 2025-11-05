use std::fmt;

/// Represents the author/source of a message
#[derive(Debug, Clone)]
pub struct MessageAuthor {
    pub nick: String,
    pub host: Option<String>,
    pub channel: String,
}

impl MessageAuthor {
    pub fn new(nick: String, channel: String) -> Self {
        Self {
            nick,
            host: None,
            channel,
        }
    }

    pub fn with_host(mut self, host: String) -> Self {
        self.host = Some(host);
        self
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

    /// Shutdown the plugin
    Shutdown,
}

/// Responses from plugins
#[derive(Debug, Clone)]
pub enum PluginResponse {
    /// Result of TCL evaluation
    TclResult { original_message: Message, output: String },

    /// Error occurred
    Error { message: String },
}
