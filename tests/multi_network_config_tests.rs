use slopdrop::config::{Config, ServerConfig};
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to create a temporary config file with given TOML content
fn write_temp_config(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

// =============================================================================
// Config Parsing: [server] backwards compatibility
// =============================================================================

#[test]
fn test_single_server_config_backwards_compat() {
    let toml_content = r##"
[server]
hostname = "irc.libera.chat"
port = 6697
use_tls = true
nickname = "testbot"
channels = ["#test"]

[security]
privileged_users = ["admin!*@*"]
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].hostname, "irc.libera.chat");
    assert_eq!(servers[0].port, 6697);
    assert!(servers[0].use_tls);
    assert_eq!(servers[0].nickname, "testbot");
    assert_eq!(servers[0].channels, vec!["#test"]);
}

#[test]
fn test_single_server_network_name_defaults_to_hostname() {
    let server = ServerConfig {
        name: None,
        hostname: "irc.libera.chat".to_string(),
        port: 6697,
        use_tls: true,
        nickname: "bot".to_string(),
        channels: vec![],
    };
    assert_eq!(server.network_name(), "irc.libera.chat");
}

#[test]
fn test_single_server_explicit_network_name() {
    let server = ServerConfig {
        name: Some("libera".to_string()),
        hostname: "irc.libera.chat".to_string(),
        port: 6697,
        use_tls: true,
        nickname: "bot".to_string(),
        channels: vec![],
    };
    assert_eq!(server.network_name(), "libera");
}

// =============================================================================
// Config Parsing: [[servers]] multi-network
// =============================================================================

#[test]
fn test_multi_server_config() {
    let toml_content = r##"
[[servers]]
name = "libera"
hostname = "irc.libera.chat"
port = 6697
use_tls = true
nickname = "testbot"
channels = ["#test", "#dev"]

[[servers]]
name = "oftc"
hostname = "irc.oftc.net"
port = 6697
use_tls = true
nickname = "testbot"
channels = ["#project"]

[security]
privileged_users = ["admin!*@*"]
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();

    assert_eq!(servers.len(), 2);

    assert_eq!(servers[0].network_name(), "libera");
    assert_eq!(servers[0].hostname, "irc.libera.chat");
    assert_eq!(servers[0].channels, vec!["#test", "#dev"]);

    assert_eq!(servers[1].network_name(), "oftc");
    assert_eq!(servers[1].hostname, "irc.oftc.net");
    assert_eq!(servers[1].channels, vec!["#project"]);
}

#[test]
fn test_multi_server_without_name_defaults_to_hostname() {
    let toml_content = r##"
[[servers]]
hostname = "irc.libera.chat"
port = 6697
use_tls = true
nickname = "testbot"
channels = ["#test"]

[[servers]]
hostname = "irc.oftc.net"
port = 6697
use_tls = true
nickname = "testbot"
channels = ["#project"]

[security]
privileged_users = []
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();

    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].network_name(), "irc.libera.chat");
    assert_eq!(servers[1].network_name(), "irc.oftc.net");
}

#[test]
fn test_servers_takes_priority_over_server() {
    let toml_content = r##"
[server]
hostname = "should.be.ignored"
port = 6667
use_tls = false
nickname = "ignored"
channels = ["#ignored"]

[[servers]]
name = "actual"
hostname = "irc.actual.net"
port = 6697
use_tls = true
nickname = "realbot"
channels = ["#real"]

[security]
privileged_users = []
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();

    // [[servers]] should take priority over [server]
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].network_name(), "actual");
    assert_eq!(servers[0].hostname, "irc.actual.net");
}

#[test]
fn test_no_servers_returns_empty() {
    let toml_content = r##"
[security]
privileged_users = []
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();
    assert!(servers.is_empty());
}

#[test]
fn test_single_server_with_name_field() {
    let toml_content = r##"
[server]
name = "mynet"
hostname = "irc.example.com"
port = 6667
use_tls = false
nickname = "bot"
channels = ["#chan"]

[security]
privileged_users = []
eval_timeout_ms = 5000

[tcl]
state_path = "/tmp/test_state"
max_output_lines = 10
"##;
    let file = write_temp_config(toml_content);

    let config = Config::from_file(file.path().to_str().unwrap()).unwrap();
    let servers = config.get_servers();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].network_name(), "mynet");
    assert_eq!(servers[0].name, Some("mynet".to_string()));
}

// =============================================================================
// PluginCommand network field
// =============================================================================

#[test]
fn test_plugin_command_carries_network() {
    use slopdrop::types::{MessageAuthor, Message, PluginCommand};

    let author = MessageAuthor::new("nick".to_string(), "#test".to_string())
        .with_network("libera".to_string());

    assert_eq!(author.network, "libera");

    let msg = Message::new(author, "tcl expr 1+1".to_string());
    let cmd = PluginCommand::EvalTcl { message: msg, is_admin: false };

    match cmd {
        PluginCommand::EvalTcl { message, .. } => {
            assert_eq!(message.author.network, "libera");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_message_author_default_network() {
    use slopdrop::types::MessageAuthor;

    let author = MessageAuthor::new("nick".to_string(), "#test".to_string());
    assert_eq!(author.network, "default");
}

#[test]
fn test_send_to_irc_has_network() {
    use slopdrop::types::PluginCommand;

    let cmd = PluginCommand::SendToIrc {
        network: "libera".to_string(),
        channel: "#test".to_string(),
        text: "hello".to_string(),
    };

    match cmd {
        PluginCommand::SendToIrc { network, channel, text } => {
            assert_eq!(network, "libera");
            assert_eq!(channel, "#test");
            assert_eq!(text, "hello");
        }
        _ => panic!("Wrong variant"),
    }
}
