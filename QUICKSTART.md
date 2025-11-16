# Quick Start Guide

## Running the Bot Locally

### 1. Create a Config File

Copy the example configuration:
```bash
cp config.toml.example config.toml
```

Edit `config.toml` to configure your IRC server:
```toml
[server]
hostname = "irc.libera.chat"  # Your IRC server
port = 6697
use_tls = true
nickname = "mybot"
channels = ["#test"]

[security]
privileged_users = ["yournick"]  # Your IRC nick
eval_timeout_ms = 30000

[tcl]
state_path = "./state"  # Bot will auto-create this
max_output_lines = 10
```

### 2. Build and Run

```bash
# Build release version
cargo build --release

# Run the bot
./target/release/slopdrop config.toml
```

**That's it!** The bot will now:
- ✅ Auto-create the `./state` directory
- ✅ Initialize a git repository in it
- ✅ Connect to IRC and join channels
- ✅ Respond to `tcl` commands

### 3. Using the Bot

In IRC:
```
<you> tcl expr {1 + 1}
<bot> 2

<you> tcl set x "hello"
<bot> hello

<you> tcl proc greet {} { return "hi there!" }
<bot>

<you> tcl greet
<bot> hi there!
```

The bot will automatically save all procs and vars to the state directory!

## Running Tests

The tests will also auto-create their state directory:

```bash
# Run comprehensive test suite (28 tests)
./tests/comprehensive_tests.sh

# Run basic integration tests
./tests/run_integration_tests.sh
```

Tests use `/tmp/slopdrop_test_state` which is auto-created and cleaned up.

## Troubleshooting

### "Connection refused" Error

If you see:
```
Error: an io error occurred
Caused by: Connection refused (os error 111)
```

This means the bot can't connect to the IRC server. Check:
1. Is the hostname correct in `config.toml`?
2. Is the port correct (usually 6697 for TLS, 6667 for plain)?
3. Can you reach the server? Try: `telnet irc.libera.chat 6697`

### State Directory Issues

**No longer a problem!** As of this commit, the state directory is automatically created on startup.

If you ever need to reset state:
```bash
rm -rf ./state
# Bot will auto-create it on next run
```

## Advanced

### Admin Commands

For users listed in `privileged_users`:
```
<you> tclAdmin history
<bot> [shows git commit history]

<you> tclAdmin rollback <commit-hash>
<bot> Rolled back to commit abc123. Note: Restart bot to reload state.
```

### Available Commands

- **Basic TCL**: All TCL 8.6 commands (safe mode)
- **cache::*** - Persistent key-value storage
- **http::get/post/head** - HTTP requests with rate limiting
- **encoding::base64/url** - Encoding/decoding
- **sha1** - SHA1 hashing (requires tcllib)
- **history** - View git commit history (admin only)
- **rollback** - Revert to previous state (admin only)
- **chanlist #channel** - List channel members

### State Management

The bot stores all TCL state in a git repository:
```bash
cd ./state
git log  # See all changes
git show HEAD  # See last change
```

Each eval creates a git commit with your IRC nick as the author!

## NixOS Users

The test scripts use `#!/usr/bin/env bash` which works on NixOS.

If you need to run the bot on NixOS, make sure `tcl`, `git`, and `tcllib` are available in your environment.
