//! Comprehensive live IRC integration tests
//!
//! These tests start a real IRC server (ergochat), run the slopdrop bot,
//! and verify end-to-end functionality using test IRC clients.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{fs, thread};
use tokio::sync::mpsc;
use tokio::time::sleep;
use irc::client::prelude::*;
use irc::proto::Command as IrcCommand;
use futures::StreamExt;
use once_cell::sync::Lazy;

use slopdrop::config::{SecurityConfig, ServerConfig, TclConfig};
use slopdrop::irc_client::IrcClient;
use slopdrop::tcl_plugin::TclPlugin;
use slopdrop::types::ChannelMembers;

/// Global test counter for unique IDs
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Get a unique test ID
fn get_unique_test_id() -> u64 {
    TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Shared IRC server for all tests
static SHARED_SERVER: Lazy<Arc<SharedTestIrcServer>> = Lazy::new(|| {
    Arc::new(SharedTestIrcServer::start().expect("Failed to start shared IRC server"))
});

/// Shared IRC server that lives for the duration of all tests
struct SharedTestIrcServer {
    _process: Child,
}

impl SharedTestIrcServer {
    fn start() -> Result<Self, Box<dyn std::error::Error>> {
        // Clean up old test database
        let _ = fs::remove_file("/tmp/ergo-test.db");

        // Start ergo server
        let process = Command::new("./tests/ergo/ergo")
            .arg("run")
            .arg("--conf")
            .arg("./tests/ergo/test-ircd.yaml")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Give server time to start
        thread::sleep(Duration::from_secs(2));

        Ok(Self { _process: process })
    }
}


/// Helper struct to manage the full bot (IrcClient + TclPlugin)
struct TestBot {
    _irc_handle: tokio::task::JoinHandle<()>,
    _tcl_handle: tokio::task::JoinHandle<()>,
}

impl TestBot {
    async fn start_with_channel(state_path: &str, channel: &str, bot_nick: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let channel_members: ChannelMembers = Arc::new(RwLock::new(HashMap::new()));

        // Create communication channels
        let (tcl_command_tx, tcl_command_rx) = mpsc::channel(100);
        let (irc_response_tx, mut irc_response_rx) = mpsc::channel(100);

        // Create configs
        let server_config = ServerConfig {
            hostname: "127.0.0.1".to_string(),
            port: 16667,
            use_tls: false,
            nickname: bot_nick.to_string(),
            channels: vec![channel.to_string()],
        };

        let security_config = SecurityConfig {
            eval_timeout_ms: 5000,
            privileged_users: vec!["testadmin!*@*".to_string()],
            blacklisted_users: vec![],
            memory_limit_mb: 0,
            max_recursion_depth: 1000,
            notify_self: false,
        };

        let tcl_config = TclConfig {
            state_path: PathBuf::from(state_path),
            state_repo: None,
            max_output_lines: 10,
            ssh_key: None,
        };

        // Spawn TCL plugin
        let channel_members_clone = channel_members.clone();
        let server_config_clone = server_config.clone();
        let tcl_handle = tokio::task::spawn_blocking(move || {
            let mut tcl_plugin = match TclPlugin::new(
                security_config,
                tcl_config,
                server_config_clone,
                PathBuf::from("/tmp/test_config.toml"),
                channel_members_clone,
            ) {
                Ok(plugin) => plugin,
                Err(e) => {
                    eprintln!("Failed to create TCL plugin: {}", e);
                    return;
                }
            };

            // Create a dedicated runtime for the TCL plugin
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(async {
                if let Err(e) = tcl_plugin.run(tcl_command_rx, irc_response_tx, None).await {
                    eprintln!("TCL plugin error: {}", e);
                }
            });
        });

        // Spawn IRC client
        let irc_handle = tokio::spawn(async move {
            match IrcClient::new(server_config, channel_members).await {
                Ok(irc_client) => {
                    if let Err(e) = irc_client.run(tcl_command_tx, &mut irc_response_rx).await {
                        eprintln!("IRC client error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create IRC client: {}", e);
                }
            }
        });

        // Give bot time to connect and join channels
        sleep(Duration::from_secs(4)).await;

        Ok(Self {
            _irc_handle: irc_handle,
            _tcl_handle: tcl_handle,
        })
    }
}

/// Helper to create a test IRC client that's ready to send messages
async fn create_test_client_with_channel(nick: &str, channel: &str) -> Result<(Client, irc::client::ClientStream), Box<dyn std::error::Error>> {
    let config = Config {
        nickname: Some(nick.to_string()),
        server: Some("127.0.0.1".to_string()),
        port: Some(16667),
        channels: vec![channel.to_string()],
        use_tls: Some(false),
        ..Default::default()
    };

    let mut client = Client::from_config(config).await?;
    let mut stream = client.stream()?;
    client.identify()?;

    let target_channel = channel.to_string();

    // Wait for JOIN to complete by consuming messages until we see it
    let timeout = sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(Ok(message)) = stream.next() => {
                if let IrcCommand::JOIN(joined_channel, _, _) = message.command {
                    if joined_channel == target_channel {
                        break;
                    }
                }
            }
            _ = &mut timeout => {
                return Err(format!("Timeout waiting for JOIN to {}", channel).into());
            }
        }
    }

    Ok((client, stream))
}

/// Helper to wait for a specific response from the bot
/// Collects up to 3 responses and returns the first non-TIMTOM message
/// (TIMTOM greets 70% of the time, so we need to handle both cases)
async fn wait_for_response_from(
    stream: &mut irc::client::ClientStream,
    timeout_secs: u64,
    channel: &str,
    bot_nick: &str,
) -> Option<String> {
    // Collect up to 3 responses in case TIMTOM greets
    let responses = wait_for_responses_from(stream, 3, timeout_secs, channel, bot_nick).await;

    // Return the first non-TIMTOM response
    for response in &responses {
        if !response.contains("Welcome to") && !response.contains("TIMTOM is here to serve you") {
            return Some(response.clone());
        }
    }

    // If all were TIMTOM greetings, return the last one
    responses.last().cloned()
}

/// Helper to wait for multiple responses
async fn wait_for_responses_from(
    stream: &mut irc::client::ClientStream,
    expected_count: usize,
    timeout_secs: u64,
    channel: &str,
    bot_nick: &str,
) -> Vec<String> {
    let mut responses = Vec::new();
    let timeout = sleep(Duration::from_secs(timeout_secs));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(Ok(message)) = stream.next() => {
                if let IrcCommand::PRIVMSG(target, msg) = message.command {
                    if target == channel {
                        if let Some(irc::proto::Prefix::Nickname(nick, _, _)) = message.prefix {
                            if nick == bot_nick {
                                responses.push(msg);
                                if responses.len() >= expected_count {
                                    return responses;
                                }
                            }
                        }
                    }
                }
            }
            _ = &mut timeout => {
                return responses;
            }
        }
    }
}

// =============================================================================
// Basic TCL Evaluation Tests
// =============================================================================

#[tokio::test]
async fn test_live_basic_tcl_eval() {
    // Ensure shared server is running
    let _server = Lazy::force(&SHARED_SERVER);

    // Generate unique identifiers for this test
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);

    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick)
        .await
        .expect("Failed to start bot");

    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel)
        .await
        .expect("Failed to connect");

    // Test basic expression
    client.send_privmsg(&channel, "tcl expr {1 + 1}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "2", "Expected '2' but got '{}'", response);
    } else {
        panic!("No response received for basic tcl eval");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_string_operations() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    client.send_privmsg(&channel, "tcl string toupper hello").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "HELLO");
    } else {
        panic!("No response received for string toupper");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_list_operations() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test list operations
    client.send_privmsg(&channel, "tcl llength {a b c d e}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "5");
    } else {
        panic!("No response received for llength");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_math_operations() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test math expression
    client.send_privmsg(&channel, "tcl expr {sqrt(16) + pow(2, 3)}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        let value: f64 = response.trim().parse().expect("Not a number");
        assert!((value - 12.0).abs() < 0.001, "Expected 12.0, got {}", value);
    } else {
        panic!("No response received for math operations");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Proc Definition and State Persistence Tests
// =============================================================================

#[tokio::test]
async fn test_live_define_proc() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Define a proc
    client.send_privmsg(&channel, "tcl proc double {x} { expr {$x * 2} }").expect("Failed to send");

    // Wait for definition confirmation (or empty response)
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    // Now call the proc
    client.send_privmsg(&channel, "tcl double 21").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "42");
    } else {
        panic!("No response received for proc call");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_variable_persistence() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Set a variable
    client.send_privmsg(&channel, "tcl set testvar 12345").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "12345");
    } else {
        panic!("No response received for set");
    }

    // Get the variable back
    client.send_privmsg(&channel, "tcl set testvar").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "12345");
    } else {
        panic!("No response received for get");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Utility Commands Tests
// =============================================================================

#[tokio::test]
async fn test_live_map_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test map command
    client.send_privmsg(&channel, "tcl map {1 2 3} {x {expr {$x * 2}}}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "2 4 6");
    } else {
        panic!("No response received for map");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_seq_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test seq command
    client.send_privmsg(&channel, "tcl seq 1 5").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "1 2 3 4 5");
    } else {
        panic!("No response received for seq");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_first_last_rest() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test first
    client.send_privmsg(&channel, "tcl first {a b c}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "a");
    } else {
        panic!("No response received for first");
    }

    // Test last
    client.send_privmsg(&channel, "tcl last {a b c}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "c");
    } else {
        panic!("No response received for last");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Cache System Tests
// =============================================================================

#[tokio::test]
async fn test_live_cache_put_get() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Put value in cache
    client.send_privmsg(&channel, "tcl cache put mybucket mykey myvalue").expect("Failed to send");
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    // Get value from cache
    client.send_privmsg(&channel, "tcl cache get mybucket mykey").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "myvalue");
    } else {
        panic!("No response received for cache get");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_cache_exists() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Check non-existent key
    client.send_privmsg(&channel, "tcl cache exists mybucket nonexistent").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "0");
    } else {
        panic!("No response received for cache exists");
    }

    // Put value
    client.send_privmsg(&channel, "tcl cache put mybucket testkey testvalue").expect("Failed to send");
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    // Check existent key
    client.send_privmsg(&channel, "tcl cache exists mybucket testkey").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "1");
    } else {
        panic!("No response received for cache exists after put");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Encoding Tests
// =============================================================================

#[tokio::test]
async fn test_live_base64_encode_decode() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Base64 encode
    client.send_privmsg(&channel, "tcl base64 {hello world}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "aGVsbG8gd29ybGQ=");
    } else {
        panic!("No response received for base64");
    }

    // Base64 decode
    client.send_privmsg(&channel, "tcl unbase64 aGVsbG8gd29ybGQ=").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "hello world");
    } else {
        panic!("No response received for unbase64");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_url_encode_decode() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // URL encode
    client.send_privmsg(&channel, "tcl url_encode {hello world!}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "hello%20world%21");
    } else {
        panic!("No response received for url_encode");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_live_syntax_error() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Unbalanced brackets
    client.send_privmsg(&channel, "tcl expr {1 + 1").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("error"), "Expected error message, got: {}", response);
    } else {
        panic!("No response received for syntax error");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_undefined_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Undefined command
    client.send_privmsg(&channel, "tcl nonexistentcommand").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("error") || response.contains("invalid command"),
                "Expected error message, got: {}", response);
    } else {
        panic!("No response received for undefined command");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Security Tests
// =============================================================================

#[tokio::test]
async fn test_live_blocked_exec_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Try to execute system command (should be blocked)
    client.send_privmsg(&channel, "tcl exec ls").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("error") || response.contains("invalid command"),
                "exec should be blocked, got: {}", response);
    } else {
        panic!("No response received for blocked exec");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_blocked_socket_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Try to open socket (should be blocked)
    client.send_privmsg(&channel, "tcl socket localhost 80").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("error") || response.contains("invalid command"),
                "socket should be blocked, got: {}", response);
    } else {
        panic!("No response received for blocked socket");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// File Path Operations Tests (Safe file command)
// =============================================================================

#[tokio::test]
async fn test_live_file_join() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test file join
    client.send_privmsg(&channel, "tcl file join /home user docs").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "/home/user/docs");
    } else {
        panic!("No response received for file join");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_file_extension() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test file extension
    client.send_privmsg(&channel, "tcl file extension /path/to/file.txt").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), ".txt");
    } else {
        panic!("No response received for file extension");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_file_dirname() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test file dirname
    client.send_privmsg(&channel, "tcl file dirname /path/to/file.txt").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "/path/to");
    } else {
        panic!("No response received for file dirname");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Triggers Tests
// =============================================================================

#[tokio::test]
async fn test_live_trigger_registration() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Register a trigger using the bind command
    // triggers bind <event> <pattern> <proc>
    // First define the proc
    client.send_privmsg(&channel, "tcl proc myhandler {nick mask chan text} { return \"Got text from $nick\" }").expect("Failed to send");
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    // Now bind it
    client.send_privmsg(&channel, "tcl triggers bind TEXT #test myhandler").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        // Should get a success message
        assert!(response.contains("Bound"), "Expected bind success message, got: {}", response);
    } else {
        panic!("No response received for triggers bind");
    }

    // List triggers
    client.send_privmsg(&channel, "tcl triggers list_bindings").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        // Should contain TEXT and #test
        assert!(response.contains("TEXT") || response.contains("myhandler"),
                "Expected trigger list to contain binding info, got: {}", response);
    } else {
        panic!("No response received for triggers list_bindings");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Timer Tests
// =============================================================================

#[tokio::test]
async fn test_live_timer_schedule() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // List timers (should be empty or just show format)
    client.send_privmsg(&channel, "tcl timers list").expect("Failed to send");

    // Just verify we get a response (not an error)
    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        // Response should be empty list or timer list
        assert!(!response.contains("invalid command"), "timers command should exist");
    } else {
        panic!("No response received for timers list");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Context Variable Tests
// =============================================================================

#[tokio::test]
async fn test_live_nick_variable() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Get nick variable (should be the sender's nick)
    client.send_privmsg(&channel, "tcl nick").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), client_nick);
    } else {
        panic!("No response received for nick");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_channel_variable() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Get channel variable
    client.send_privmsg(&channel, "tcl set ::channel").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), channel);
    } else {
        panic!("No response received for channel");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Select/Filter Tests
// =============================================================================

#[tokio::test]
async fn test_live_select_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test select command
    client.send_privmsg(&channel, "tcl select {1 2 3 4 5} {x {expr {$x > 2}}}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "3 4 5");
    } else {
        panic!("No response received for select");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_lfilter_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test lfilter command
    client.send_privmsg(&channel, "tcl lfilter {test*} {testing test123 other testbot}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("testing"), "Expected 'testing' in result: {}", response);
        assert!(response.contains("test123"), "Expected 'test123' in result: {}", response);
        assert!(response.contains("testbot"), "Expected 'testbot' in result: {}", response);
        assert!(!response.contains("other"), "Should not contain 'other': {}", response);
    } else {
        panic!("No response received for lfilter");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Multiple Command Sequence Tests
// =============================================================================

#[tokio::test]
async fn test_live_command_sequence() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Sequence of commands building on each other
    client.send_privmsg(&channel, "tcl set x 10").expect("Failed to send");
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    client.send_privmsg(&channel, "tcl set y 20").expect("Failed to send");
    let _ = wait_for_response_from(&mut stream, 5, &channel, &bot_nick).await;

    client.send_privmsg(&channel, "tcl expr {$x + $y}").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "30");
    } else {
        panic!("No response received for expression with variables");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Meta Namespace Tests
// =============================================================================

#[tokio::test]
async fn test_live_meta_uptime() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Get uptime
    client.send_privmsg(&channel, "tcl meta uptime").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        // Should be a number >= 0
        let uptime: i64 = response.trim().parse().expect("Uptime should be a number");
        assert!(uptime >= 0, "Uptime should be non-negative");
    } else {
        panic!("No response received for meta uptime");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Clock Command Tests
// =============================================================================

#[tokio::test]
async fn test_live_clock_seconds() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Get clock seconds
    client.send_privmsg(&channel, "tcl clock seconds").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        let seconds: i64 = response.trim().parse().expect("Clock seconds should be a number");
        // Should be a reasonable timestamp (after 2020)
        assert!(seconds > 1577836800, "Timestamp should be after 2020");
    } else {
        panic!("No response received for clock seconds");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// HTTP Module Configuration Tests
// =============================================================================

#[tokio::test]
async fn test_live_httpx_namespace() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Check httpx namespace exists
    client.send_privmsg(&channel, "tcl namespace exists ::httpx").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "1");
    } else {
        panic!("No response received for httpx namespace check");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_httpx_normalize_url() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test URL normalization
    client.send_privmsg(&channel, "tcl ::httpx::normalize_url example.com").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "http://example.com");
    } else {
        panic!("No response received for URL normalization");
    }

    let _ = fs::remove_dir_all(&state_path);
}

// =============================================================================
// Link Resolver Integration Tests
// =============================================================================

#[tokio::test]
async fn test_live_linkresolver_enable_disable() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test enable
    client.send_privmsg(&channel, "tcl linkresolver enable").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("enabled"));
    } else {
        panic!("No response received for linkresolver enable");
    }

    // Test disable
    client.send_privmsg(&channel, "tcl linkresolver disable").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("disabled"));
    } else {
        panic!("No response received for linkresolver disable");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_register_custom_resolver() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Create a test resolver
    client.send_privmsg(&channel, r#"tcl proc test_resolver {url nick channel} { return "TEST: $url" }"#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    // Register the resolver
    client.send_privmsg(&channel, r#"tcl linkresolver register {example\.com} test_resolver"#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("Registered resolver"));
        assert!(response.contains("example"));
    } else {
        panic!("No response received for linkresolver register");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_list() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // List resolvers (should show built-in ones)
    client.send_privmsg(&channel, "tcl linkresolver list").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        // Should show YouTube and Bluesky resolvers that are auto-registered
        assert!(response.contains("youtube") || response.contains("bsky") || response.contains("Registered resolvers"));
    } else {
        panic!("No response received for linkresolver list");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_extract_urls() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test URL extraction
    client.send_privmsg(&channel, r#"tcl ::linkresolver::extract_urls "Check out http://example.com/test""#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("http://example.com/test"));
    } else {
        panic!("No response received for URL extraction");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_test_command() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Create a test resolver
    client.send_privmsg(&channel, r#"tcl proc my_test_resolver {url nick channel} { return "Resolved: $url" }"#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    // Register it
    client.send_privmsg(&channel, r#"tcl linkresolver register {testsite\.com} my_test_resolver"#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    // Test resolution
    client.send_privmsg(&channel, r#"tcl linkresolver test "http://testsite.com/page""#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("Resolved: http://testsite.com/page"));
    } else {
        panic!("No response received for linkresolver test");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_caching() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Set cached value
    client.send_privmsg(&channel, r#"tcl ::linkresolver::set_cached "http://example.com" "Cached result""#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    // Get cached value
    client.send_privmsg(&channel, r#"tcl ::linkresolver::get_cached "http://example.com""#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "Cached result");
    } else {
        panic!("No response received for get_cached");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_html_entity_decoding() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test HTML entity decoding
    client.send_privmsg(&channel, r#"tcl ::linkresolver::decode_html_entities "A &amp; B &lt; C""#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert_eq!(response.trim(), "A & B < C");
    } else {
        panic!("No response received for HTML entity decoding");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_format_number() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Test number formatting
    client.send_privmsg(&channel, "tcl ::linkresolver::format_number 1500000").expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("1M") || response.contains("1500K"));
    } else {
        panic!("No response received for format_number");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_find_resolver() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Find resolver for YouTube URL (should match built-in resolver)
    client.send_privmsg(&channel, r#"tcl ::linkresolver::find_resolver "https://www.youtube.com/watch?v=test""#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("youtube_resolver"));
    } else {
        panic!("No response received for find_resolver");
    }

    let _ = fs::remove_dir_all(&state_path);
}

#[tokio::test]
async fn test_live_linkresolver_unregister() {
    let _server = Lazy::force(&SHARED_SERVER);
    let test_id = get_unique_test_id();
    let channel = format!("#test{}", test_id);
    let bot_nick = format!("bot{}", test_id);
    let client_nick = format!("user{}", test_id);
    let state_path = format!("/tmp/slopdrop_test_{}", test_id);
    let _ = fs::remove_dir_all(&state_path);
    fs::create_dir_all(&state_path).expect("Failed to create state directory");

    let _bot = TestBot::start_with_channel(&state_path, &channel, &bot_nick).await.expect("Failed to start bot");
    let (client, mut stream) = create_test_client_with_channel(&client_nick, &channel).await.expect("Failed to connect");

    // Create and register a test resolver
    client.send_privmsg(&channel, r#"tcl proc temp_resolver {url nick channel} { return "temp" }"#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    client.send_privmsg(&channel, r#"tcl linkresolver register {tempsite\.com} temp_resolver"#).expect("Failed to send");
    wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await;

    // Unregister it
    client.send_privmsg(&channel, r#"tcl linkresolver unregister {tempsite\.com}"#).expect("Failed to send");

    if let Some(response) = wait_for_response_from(&mut stream, 10, &channel, &bot_nick).await {
        assert!(response.contains("Unregistered"));
    } else {
        panic!("No response received for linkresolver unregister");
    }

    let _ = fs::remove_dir_all(&state_path);
}
