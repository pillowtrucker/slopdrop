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
privileged_users = ["admin"]
eval_timeout_ms = 30000

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
- `tclAdmin <code>` - Evaluate TCL code with admin privileges (privileged users only)

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

The TCL interpreter is sandboxed by:
- Disabling dangerous commands (exec, open, file, socket, source, load, cd, pwd, glob, exit)
- Bracket balancing validation before evaluation
- Timeout protection (30s default)
- Privilege checking for admin commands
- Output line limiting

## Differences from Original

This rewrite modernizes the original Haskell+TCL implementation:

- **Language**: Rust instead of Haskell for better performance and easier deployment
- **IRC Library**: Modern `irc` crate (v1.1) with full async support
- **TCL Integration**: Uses `tcltk` crate from oooutlk/tcltk
- **Architecture**: Tokio-based async instead of STM
- **Configuration**: TOML instead of INI

##TODO / Missing Features

- [ ] Git-based state persistence (versioned_interpreter)
- [ ] Auto-rejoin on kick (needs restructuring)
- [ ] Timeout mechanism (SIGALRM equivalent)
- [ ] User-defined proc tracking and persistence
- [ ] IRC formatting handling (colors, bold, etc.)
- [ ] Channel member list tracking

## License

AGPL-3 (matching original)

## Credits

Original bot: https://github.com/pillowtrucker/old-tcl-evalbot
TCL bindings: https://github.com/oooutlk/tcltk
