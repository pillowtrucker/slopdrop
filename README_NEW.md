# Slopdrop - Multi-Frontend TCL Evalbot

A modern, multi-frontend TCL evaluation platform written in Rust. Evaluate TCL code from **IRC, CLI, TUI, or Web** - all sharing the same interpreter and state!

```ascii
┌──────────────────────────────────────────────────────────┐
│                   Choose Your Interface                   │
├─────────────┬────────────┬────────────┬──────────────────┤
│     IRC     │    CLI     │    TUI     │       Web        │
│  Chat bot   │   REPL     │ Terminal   │  Browser + API   │
└─────────────┴────────────┴────────────┴──────────────────┘
                            │
                      ┌─────▼──────┐
                      │ TCL Engine │
                      │ Git State  │
                      └────────────┘
```

## 🚀 Quick Start

```bash
# Build with all frontends
cargo build --release --features all-frontends

# IRC bot (default)
./target/release/slopdrop

# Interactive CLI
./target/release/slopdrop --cli

# Full-screen TUI
./target/release/slopdrop --tui

# Web server + API
./target/release/slopdrop --web
# Then open http://127.0.0.1:8080

# Multiple at once!
./target/release/slopdrop --irc --web
```

## ✨ Features

### 🌐 **4 Frontends, 1 Backend**

| Frontend | Use Case | Interface | Best For |
|----------|----------|-----------|----------|
| **IRC** | Team chat bot | IRC channels | Collaboration, public bots |
| **CLI** | Command-line REPL | Terminal stdin/stdout | Quick testing, scripting |
| **TUI** | Full-screen UI | Terminal (ratatui) | Development, debugging |
| **Web** | Browser + REST API | HTTP + JSON | Remote access, integration |

### 🔐 **Security**
- ✅ Sandboxed TCL interpreter (exec, file, socket disabled)
- ✅ Timeout protection (30s default, configurable)
- ✅ Hostmask-based admin authentication (IRC)
- ✅ Bracket balancing validation
- ✅ Automatic thread restart on timeout

### 💾 **State Persistence**
- ✅ Git-backed state versioning
- ✅ Every change auto-committed
- ✅ Full git history (`tclAdmin history`)
- ✅ Rollback to any commit (`tclAdmin rollback <hash>`)
- ✅ Optional remote git push (SSH/HTTPS)
- ✅ PM notifications to admins on commits

### ⚡ **Performance**
- ✅ Async architecture (Tokio)
- ✅ Thread-based TCL evaluation
- ✅ Efficient message routing
- ✅ Output pagination (configurable)

### 🎨 **User Experience**
- ✅ ANSI color support
- ✅ Smart message splitting
- ✅ HTTP command support (`http <url>`)
- ✅ Emulated smeggdrop commands
- ✅ Persistent command history (CLI)
- ✅ Keyboard shortcuts (TUI/Web)

## 📦 Installation

### Prerequisites

- **Rust** 1.70+ ([rustup](https://rustup.rs/))
- **TCL 8.6** development headers
- **pkg-config**

#### Ubuntu/Debian
```bash
apt-get install tcl8.6-dev pkg-config
```

#### Arch Linux
```bash
pacman -S tcl pkg-config
```

#### macOS
```bash
brew install tcl-tk pkg-config
```

### Build

```bash
# Clone repository
git clone https://github.com/yourusername/slopdrop.git
cd slopdrop

# Set TCL environment (if needed)
export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_PATH
export TCL_INCLUDE_PATH=/usr/include/tcl8.6
export TCL_LIBRARY=/usr/lib/x86_64-linux-gnu/libtcl8.6.so

# Build with all frontends
cargo build --release --features all-frontends

# Or build specific frontends
cargo build --release --features frontend-cli
cargo build --release --features frontend-tui
cargo build --release --features frontend-web
```

## 🎯 Usage

### IRC Frontend

**Classic IRC bot interface** - the original slopdrop experience.

```bash
# Create config.toml (see config.toml.example)
./slopdrop
```

**In IRC:**
```
<user> tcl expr {1 + 1}
<bot> 2

<user> tcl set myvar "hello world"
<bot> hello world

<admin> tclAdmin history
<bot> abc1234 - alice - Evaluated set myvar...
<bot> def5678 - bob - Evaluated proc greet...

<admin> tclAdmin rollback abc1234
<bot> Rolled back to commit abc1234
```

### CLI Frontend

**Interactive command-line REPL** with readline support.

```bash
./slopdrop --cli
```

```tcl
slopdrop> expr {1 + 1}
2

slopdrop> proc factorial {n} {
>     if {$n <= 1} { return 1 }
>     expr {$n * [factorial [expr {$n - 1}]]}
> }

slopdrop> factorial 5
120

slopdrop> .history
Git History:
  abc1234 - alice - Evaluated proc factorial...
  def5678 - alice - Evaluated expr {1 + 1}

slopdrop> .quit
```

**Special commands:**
- `.help` - Show help
- `.history [N]` - Show last N commits
- `.rollback <hash>` - Rollback to commit
- `.more` - Get more paginated output
- `.quit` / `.exit` - Exit

### TUI Frontend

**Full-screen terminal UI** with split panes.

```bash
./slopdrop --tui
```

**Layout:**
```
┌─────────────────────────────────────────────────┐
│ Output:                                         │
│ > expr {1 + 1}                                  │
│ 2                                               │
├─────────────────────────────────────────────────┤
│ Input: (Ctrl+Enter to eval, Ctrl+C to quit)    │
│ _                                               │
├─────────────────────────────────────────────────┤
│ Git History:                                    │
│ abc1234 - alice - Evaluated expr {1 + 1}        │
├─────────────────────────────────────────────────┤
│ Status: Ready                                   │
└─────────────────────────────────────────────────┘
```

**Keyboard shortcuts:**
- `Ctrl+Enter` - Evaluate code
- `Ctrl+C` - Quit
- `F2` - Get more output
- `F3` - Refresh git history

### Web Frontend

**Browser interface + REST API**

```bash
./slopdrop --web
# Open http://127.0.0.1:8080
```

**REST API:**
```bash
# Evaluate code
curl -X POST http://localhost:8080/api/eval \
  -H 'Content-Type: application/json' \
  -d '{"code":"expr {1 + 1}","is_admin":true}'

# Get history
curl http://localhost:8080/api/history

# Rollback
curl -X POST http://localhost:8080/api/rollback \
  -H 'Content-Type: application/json' \
  -d '{"commit_hash":"abc1234"}'
```

**Web UI features:**
- Monaco-style code editor
- Real-time output display
- Git history sidebar
- Click-to-rollback
- Keyboard shortcuts (Ctrl+Enter, Ctrl+L)

### Multiple Frontends

Run multiple frontends **simultaneously**!

```bash
# IRC bot + Web admin interface
./slopdrop --irc --web

# CLI + Web (testing and API access)
./slopdrop --cli --web

# TUI + Web (development setup)
./slopdrop --tui --web
```

## ⚙️ Configuration

Create `config.toml` from `config.toml.example`:

```toml
[server]
hostname = "irc.libera.chat"
port = 6697
use_tls = true
nickname = "slopdrop"
channels = ["#mychannel"]

[security]
eval_timeout_ms = 30000
privileged_users = [
    "alice!*@*.example.com",
    "bob!~bob@*"
]

[tcl]
state_path = "./state"
state_repo = "git@github.com:user/repo.git"  # Optional
ssh_key = "/home/user/.ssh/id_rsa"          # Optional
max_output_lines = 10
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with live IRC tests (requires Ergo server)
cargo test -- --include-ignored

# Run specific test suite
cargo test --test state_persistence_tests
cargo test --test tcl_evaluation_tests

# Test CLI frontend
cargo test --features frontend-cli

# Test all frontends
cargo test --features all-frontends
```

**Test coverage:**
- ✅ 89 tests total
- ✅ 100% success rate
- ✅ Unit tests (16)
- ✅ Integration tests (73)
- ✅ State persistence
- ✅ TCL evaluation
- ✅ Timeout protection
- ✅ PM notifications
- ✅ Output pagination
- ✅ Live IRC integration

## 📚 Documentation

- **[FRONTEND_GUIDE.md](FRONTEND_GUIDE.md)** - Complete frontend usage guide
- **[MULTI_FRONTEND_DESIGN.md](MULTI_FRONTEND_DESIGN.md)** - Architecture documentation
- **[TESTING_GUIDE.md](TESTING_GUIDE.md)** - Testing instructions
- **[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)** - Implementation details
- **[config.toml.example](config.toml.example)** - Configuration template

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────┐
│              Frontend Layer                      │
├──────────┬──────────┬──────────┬────────────────┤
│   IRC    │   CLI    │   TUI    │      Web       │
│ (irc)    │ (rust-   │ (rata-   │ (axum)         │
│          │  tyline) │   tui)   │                │
└────┬─────┴────┬─────┴────┬─────┴────┬───────────┘
     │          │          │          │
     └──────────┴──────────┴──────────┘
                    │
         ┌──────────▼──────────┐
         │   TclService        │
         │  (Frontend-agnostic)│
         └──────────┬──────────┘
                    │
         ┌──────────┴──────────┐
         │  TclThreadHandle    │
         │  (Thread-safe eval) │
         └──────────┬──────────┘
                    │
         ┌──────────┴──────────┐
         │  SafeTclInterp      │
         │  (Sandboxed TCL)    │
         └──────────┬──────────┘
                    │
         ┌──────────┴──────────┐
         │  StatePersistence   │
         │  (Git versioning)   │
         └─────────────────────┘
```

## 🔧 Development

### Adding a New Frontend

1. Create module in `src/frontends/your_frontend.rs`
2. Implement `Frontend` trait from `src/frontend.rs`
3. Use `TclService` for TCL evaluation
4. Add feature flag to `Cargo.toml`
5. Update `main.rs` to handle new frontend
6. Add documentation and tests

**Example frontend template:**

```rust
use crate::frontend::Frontend;
use crate::tcl_service::TclService;
use async_trait::async_trait;

pub struct MyFrontend {
    tcl_service: TclService,
    running: bool,
}

#[async_trait]
impl Frontend for MyFrontend {
    fn name(&self) -> &str { "MyFrontend" }

    async fn start(&mut self) -> Result<()> {
        self.running = true;
        // Your frontend logic here
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.running = false;
        self.tcl_service.shutdown();
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }
}
```

### Project Structure

```
slopdrop/
├── src/
│   ├── main.rs              # Multi-frontend entry point
│   ├── config.rs            # Configuration
│   ├── frontend.rs          # Frontend trait
│   ├── tcl_service.rs       # Core TCL service
│   ├── tcl_thread.rs        # Thread-safe TCL eval
│   ├── tcl_wrapper.rs       # Sandboxed interpreter
│   ├── state.rs             # Git state persistence
│   ├── frontends/
│   │   ├── mod.rs
│   │   ├── cli.rs           # CLI REPL frontend
│   │   ├── tui.rs           # TUI frontend
│   │   └── web.rs           # Web frontend
│   ├── irc_client.rs        # IRC client (existing)
│   ├── tcl_plugin.rs        # IRC TCL plugin (existing)
│   └── ...
├── tests/                   # Integration tests
├── tcl/                     # TCL library scripts
└── docs/                    # Documentation
```

## 🤝 Contributing

Contributions welcome! Areas for improvement:

- [ ] WebSocket support for web frontend
- [ ] Authentication for web frontend
- [ ] Tab completion for CLI
- [ ] Syntax highlighting for CLI/TUI
- [ ] Discord frontend
- [ ] Slack frontend
- [ ] Matrix bridge
- [ ] gRPC API
- [ ] Mobile app (using web API)
- [ ] Configuration UI
- [ ] Plugin system

## 📄 License

See LICENSE file for details.

## 🙏 Acknowledgments

- Original slopdrop TCL bot authors
- Rust IRC crate maintainers
- TCL/Tk development team
- Ratatui TUI framework
- Axum web framework

## 📞 Support

- GitHub Issues: Report bugs and request features
- Documentation: See docs/ directory
- Examples: See examples/ directory

---

**Made with ❤️ and Rust** 🦀
