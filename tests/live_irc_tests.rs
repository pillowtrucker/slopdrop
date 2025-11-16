use std::process::{Command, Child, Stdio};
use std::time::Duration;
use std::thread;
use std::path::PathBuf;
use std::fs;
use tokio::time::sleep;
use irc::client::prelude::*;

/// Helper to start the Ergo IRC server for testing
struct TestIrcServer {
    process: Child,
}

impl TestIrcServer {
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

        Ok(Self { process })
    }
}

impl Drop for TestIrcServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        // Clean up test database
        let _ = fs::remove_file("/tmp/ergo-test.db");
    }
}

/// Helper to create a test IRC client
async fn create_test_client(nick: &str) -> Result<Client, Box<dyn std::error::Error>> {
    let config = Config {
        nickname: Some(nick.to_string()),
        server: Some("127.0.0.1".to_string()),
        port: Some(16667),
        channels: vec!["#test".to_string()],
        use_tls: Some(false), // Disable TLS for test server
        ..Default::default()
    };

    let client = Client::from_config(config).await?;
    client.identify()?;

    // Give time for connection
    sleep(Duration::from_millis(500)).await;

    Ok(client)
}

#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn test_live_irc_basic_connection() {
    let _server = TestIrcServer::start().expect("Failed to start IRC server");

    let client = create_test_client("testbot").await.expect("Failed to connect");

    // Send a simple message
    client.send_privmsg("#test", "Hello, world!").expect("Failed to send message");

    // Give time for message to be processed
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn test_live_irc_tcl_evaluation() {
    let _server = TestIrcServer::start().expect("Failed to start IRC server");

    // Start the bot in a separate task
    // Note: This would require refactoring main.rs to be testable
    // For now, this is a placeholder for the structure

    let client = create_test_client("testclient").await.expect("Failed to connect");

    // Send TCL command
    client.send_privmsg("#test", "tcl expr {1 + 1}").expect("Failed to send");

    sleep(Duration::from_secs(1)).await;

    // In a full implementation, we would:
    // 1. Start the bot
    // 2. Send commands
    // 3. Capture responses
    // 4. Verify output
}

#[test]
#[ignore] // Run with: cargo test --ignored
fn test_ergo_binary_exists() {
    let ergo_path = PathBuf::from("./tests/ergo/ergo");
    assert!(ergo_path.exists(), "Ergo binary not found at {:?}", ergo_path);

    // Check it's executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&ergo_path).expect("Cannot read ergo metadata");
        let permissions = metadata.permissions();
        assert!(permissions.mode() & 0o111 != 0, "Ergo binary is not executable");
    }
}

#[test]
#[ignore] // Run with: cargo test --ignored
fn test_config_files_exist() {
    assert!(PathBuf::from("./tests/test_config.toml").exists(), "test_config.toml not found");
    assert!(PathBuf::from("./tests/ergo/test-ircd.yaml").exists(), "test-ircd.yaml not found");
}
