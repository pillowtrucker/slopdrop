# Slopdrop - TCL Eval Bot (Rust Rewrite)

A modern rewrite of the TCL eval bot in Rust, providing safe TCL code evaluation over IRC.

## Features

- **IRC Integration**: Full IRC client using the `irc` crate (v1.1)
- **Safe TCL Interpreter**: Sandboxed TCL 8.6 interpreter with dangerous commands disabled
- **Security Features**:
  - Bracket balancing validation
  - Privileged user authentication
  - Command sandboxing (exec, file, socket, etc. are disabled)
  - Separate admin and user execution modes
  - **Memory limits** (Unix): Configurable per-evaluation memory caps
  - **Timeout protection**: 30s default timeout with automatic thread restart
  - **Crash recovery**: Automatic thread restart on OOM/panic
- **Async Architecture**: Built on Tokio for high-performance concurrent operations
- **Message Routing**: Efficient plugin-based architecture with mpsc channels

## Building

### Prerequisites

- Rust 1.70+
- TCL 8.6 development headers
- pkg-config

On Ubuntu/Debian:
```bash
apt-get install tcl8.6-dev pkg-config
```

### Build

```bash
# Set TCL environment variables
export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_PATH
export TCL_INCLUDE_PATH=/usr/include/tcl8.6
export TCL_LIBRARY=/usr/lib/x86_64-linux-gnu/libtcl8.6.so

# Build the project
cargo build --release
```

## Configuration

Create a `config.toml` file (see `config.toml.example`):

```toml
[server]
hostname = "irc.libera.chat"
port = 6697
use_tls = true
nickname = "slopdrop"
channels = ["#bottest"]

[security]
privileged_users = ["admin!*@trusted.example.com"]
blacklisted_users = []  # Optional: block abusive users by hostmask
eval_timeout_ms = 30000
memory_limit_mb = 256  # Unix only, 0 = no limit
max_recursion_depth = 1000  # 0 = no limit

[tcl]
state_path = "./state"
max_output_lines = 10
```

## Usage

```bash
cargo run -- config.toml
```

### Commands

- `tcl <code>` - Evaluate TCL code in sandboxed mode
- `tcl more` - Show more output from previous command (pagination)
- `tclAdmin <code>` - Evaluate TCL code with admin privileges (privileged users only)
- `tclAdmin history [n]` - View recent git commit history
- `tclAdmin rollback <commit>` - Revert state to a specific commit
- `tclAdmin blacklist list` - Show blacklisted users
- `tclAdmin blacklist add <hostmask>` - Block a user by hostmask pattern
- `tclAdmin blacklist remove <hostmask>` - Unblock a user

### Example

```
<user> tcl expr {1 + 1}
<bot> 2

<user> tcl set myvar "hello world"
<bot> hello world

<user> tcl puts $myvar
<bot> hello world
```

## Architecture

The bot consists of three main components:

1. **IRC Client** (`irc_client.rs`): Handles all IRC communication using the `irc` crate
2. **TCL Plugin** (`tcl_plugin.rs`): Processes evaluation requests
3. **TCL Wrapper** (`tcl_wrapper.rs`): Safe TCL interpreter wrapper using the `tcltk` crate

Communication between components uses Tokio mpsc channels for efficient async message passing.

## Security

The bot implements **defense-in-depth** security for safe public deployment:

### Execution Protection
- **Timeout**: 30s default (configurable)
- **Memory limit**: 256 MB on Unix systems (configurable)
- **Recursion limit**: 1000 levels (configurable)
- **Auto-restart**: Thread recovers gracefully from crashes/OOM

### Network Protection (SSRF Prevention)
- **URL validation**: Blocks localhost, private IPs, link-local addresses
- **Rate limiting**: Per-eval (5), per-user (10/min), per-channel (25/min)
- **Transfer limits**: 150KB per-request, 500KB cumulative per-eval
- **Redirect limit**: Maximum 5 redirects

### Access Control
- **User blacklist**: Block abusive users by hostmask pattern
- **Admin authentication**: Hostmask-based privilege checking
- **Input validation**: Bracket balancing, error sanitization

### Resource Limits
- **Cache limits**: 1000 keys, 100KB per value, 1MB total per bucket
- **Output pagination**: Configurable line limits
- **Sandbox**: Dangerous commands disabled (exec, open, file, socket, source, etc.)

### State Protection
- **Git versioning**: All state changes tracked with author attribution
- **Rollback**: Admin command to revert malicious changes
- **Auto GC**: Repository garbage collection every 100 commits

**For complete security documentation**, see: `PUBLIC_DEPLOYMENT_SECURITY.md`

## Differences from Original

This rewrite modernizes the original Haskell+TCL implementation:

- **Language**: Rust instead of Haskell for better performance and easier deployment
- **IRC Library**: Modern `irc` crate (v1.1) with full async support
- **TCL Integration**: Uses `tcltk` crate from oooutlk/tcltk
- **Architecture**: Tokio-based async instead of STM
- **Configuration**: TOML instead of INI

## Feature Complete! 🎉

All core features are implemented and tested:

### State Persistence
- ✅ Git-based versioned state storage
- ✅ Automatic commits with IRC user as author
- ✅ SHA1 content-addressable files
- ✅ Proc and variable tracking
- ✅ Bootstrap loading (stolen-treasure.tcl)

### Commands
- ✅ **history** - View git commit history
- ✅ **rollback** - Revert to previous state (admin only)
- ✅ **chanlist** - List channel members
- ✅ **cache::*** - Persistent key-value storage
- ✅ **http::*** - HTTP operations with rate limiting
- ✅ **encoding::*** - Base64 and URL encoding
- ✅ **sha1** - SHA1 hashing (requires tcllib)
- ✅ **Utility commands** - pick, choose, ??, first, last, rest, upper, lower

### IRC Features
- ✅ Auto-rejoin on kick (10s delay)
- ✅ Thread-based timeout with automatic restart (30s default)
- ✅ IRC color/formatting code stripping
- ✅ Smart message splitting on word boundaries
- ✅ Channel member tracking (JOIN, PART, QUIT, KICK, NICK)

### Testing
- ✅ Comprehensive test suite (28 tests, all passing)
- ✅ Integration tests with Ergo IRC server
- ✅ Automated test scripts

See `TODO.md` for optional nice-to-have features (CTCP, enhanced sandboxing, deployment tools, etc.)

## License

AGPL-3 (matching original)

## Credits

Original bot: https://github.com/pillowtrucker/old-tcl-evalbot
TCL bindings: https://github.com/oooutlk/tcltk
