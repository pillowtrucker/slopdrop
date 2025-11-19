# Slopdrop - Multi-Frontend TCL Eval Bot

A modern rewrite of the TCL eval bot in Rust, providing safe TCL code evaluation through multiple interfaces: IRC, CLI, TUI, and Web.

## Features

### Multiple Frontends
- **IRC**: Traditional IRC bot interface
- **CLI**: Interactive command-line REPL
- **TUI**: Full-screen terminal UI with split panes
- **Web**: Browser interface + REST API

All frontends share the same TCL interpreter and git-backed state.

### Core Features
- **Safe TCL Interpreter**: Sandboxed TCL 8.6 with dangerous commands disabled
- **Git State Persistence**: All changes versioned with author attribution
- **Async Architecture**: Built on Tokio for high-performance
- **Security Features**:
  - Bracket balancing validation
  - Privileged user authentication (hostmask-based)
  - Command sandboxing (exec, file, socket disabled)
  - Memory limits (Unix): Configurable per-evaluation caps
  - Timeout protection: 30s default with automatic thread restart
  - Crash recovery: Automatic restart on OOM/panic

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
# Build with all frontends
cargo build --release --features all-frontends

# Or build specific frontends
cargo build --release --features cli
cargo build --release --features tui
cargo build --release --features web
```

## Usage

### IRC Bot (Default)
```bash
./target/release/slopdrop config.toml
```

### CLI REPL
```bash
./target/release/slopdrop --cli
# Commands: .help, .history, .rollback, .more, .quit
```

### TUI (Full-screen Terminal UI)
```bash
./target/release/slopdrop --tui
# Ctrl+Enter: Evaluate | F2: More output | F3: Refresh history | Ctrl+C: Quit
```

### Web Server
```bash
./target/release/slopdrop --web
# Open http://127.0.0.1:8080
```

### Multiple Frontends
```bash
./target/release/slopdrop --irc --web  # Run both IRC and Web
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
blacklisted_users = []
eval_timeout_ms = 30000
memory_limit_mb = 256
max_recursion_depth = 1000

[tcl]
state_path = "./state"
state_repo = "git@github.com:user/state.git"  # Optional: remote git sync
ssh_key = "/path/to/key"  # Optional: SSH key for git push
max_output_lines = 10
```

## IRC Commands

- `tcl <code>` - Evaluate TCL code in sandboxed mode
- `tcl more` - Show more output from previous command (pagination)
- `tclAdmin <code>` - Evaluate with admin privileges (privileged users only)
- `tclAdmin history [n]` - View recent git commit history
- `tclAdmin rollback <commit>` - Revert state to a specific commit
- `tclAdmin blacklist list` - Show blacklisted users
- `tclAdmin blacklist add <hostmask>` - Block a user
- `tclAdmin blacklist remove <hostmask>` - Unblock a user

### Example

```
<user> tcl expr {1 + 1}
<bot> 2

<user> tcl set myvar "hello world"
<bot> hello world

<user> tcl proc greet {} { return "hi there!" }
<bot>

<user> tcl greet
<bot> hi there!
```

## Architecture

The bot uses a modular frontend architecture:

1. **Frontend Layer**: IRC, CLI, TUI, Web interfaces
2. **TCL Service** (`tcl_service.rs`): Frontend-agnostic evaluation service
3. **TCL Thread** (`tcl_thread.rs`): Thread-based execution with timeout
4. **State Persistence** (`state.rs`): Git-backed state storage

Communication uses Tokio mpsc channels for efficient async message passing.

## Security

The bot implements defense-in-depth security for safe public deployment:

### Execution Protection
- **Timeout**: 30s default (configurable)
- **Memory limit**: 256 MB on Unix (configurable)
- **Recursion limit**: 1000 levels (configurable)
- **Auto-restart**: Thread recovers from crashes/OOM

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
- **Sandbox**: Dangerous commands disabled (exec, open, file, socket, source)

### State Protection
- **Git versioning**: All changes tracked with author attribution
- **Rollback**: Admin command to revert malicious changes
- **Auto GC**: Repository garbage collection every 100 commits

**For complete security documentation**, see: `PUBLIC_DEPLOYMENT_SECURITY.md`

## Implemented Features

### State Persistence
- Git-based versioned state storage
- Automatic commits with IRC user as author
- SHA1 content-addressable files
- Proc and variable tracking
- Bootstrap loading (stolen-treasure.tcl, restore_missing_vars.tcl)
- Lazy-loaded english word list

### Commands
- **history** - View git commit history
- **rollback** - Revert to previous state (admin only)
- **chanlist** - List channel members
- **name/names** - Random/all channel members
- **cache::*** - Persistent key-value storage
- **http::*** - HTTP operations with rate limiting
- **encoding::*** - Base64 and URL encoding
- **sha1** - SHA1 hashing (requires tcllib)
- **Utility commands** - pick, choose, ??, first, last, rest, upper, lower
- **get_english_words, random_word, word_count** - English word list

### IRC Features
- Auto-rejoin on kick (10s delay)
- Thread-based timeout with automatic restart
- IRC color/formatting code stripping
- Smart message splitting on word boundaries
- Channel member tracking (JOIN, PART, QUIT, KICK, NICK)
- PM notifications to admins on commits

### Testing
- Comprehensive test suite (all passing)
- Integration tests with Ergo IRC server
- Automated test scripts

See `TODO.md` for implementation history and `TEST_COVERAGE_REPORT.md` for test details.

## Container Deployment

A Containerfile is provided for secure deployment:

```bash
# Build container
./container-build.sh

# Run with security options
./container-run.sh --config /path/to/config.toml --state /path/to/state
```

See `container-run.sh` for all options including memory limits and state reset.

## Documentation

- `QUICKSTART.md` - Quick start guide
- `FRONTEND_GUIDE.md` - Frontend usage details
- `PUBLIC_DEPLOYMENT_SECURITY.md` - Security documentation
- `HTTP_LIMITS.md` - HTTP rate limiting details
- `OOM_PROTECTION.md` - Memory protection details
- `TESTING_GUIDE.md` - Testing guide
- `tests/README.md` - Test suite documentation

## License

AGPL-3 (matching original)

## Credits

Original bot: https://github.com/pillowtrucker/old-tcl-evalbot
TCL bindings: https://github.com/oooutlk/tcltk
