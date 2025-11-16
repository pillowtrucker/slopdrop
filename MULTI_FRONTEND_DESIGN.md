# Multi-Frontend Architecture Design

## Overview

Refactor slopdrop to support multiple frontends (IRC, CLI, TUI, Web) while sharing the same TCL evaluation backend.

## Current Architecture Issues

Currently, the bot is tightly coupled to IRC:
- `main.rs` only starts IRC client
- `tcl_plugin.rs` is IRC-specific
- Output formatting assumes IRC
- No way to use TCL evaluation from other interfaces

## Proposed Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontends                             │
├──────────────┬──────────────┬──────────────┬────────────────┤
│     IRC      │   CLI/REPL   │     TUI      │   Web (HTTP)   │
│  (existing)  │  (rustyline) │  (ratatui)   │    (axum)      │
└──────┬───────┴──────┬───────┴──────┬───────┴────────┬───────┘
       │              │              │                │
       └──────────────┴──────────────┴────────────────┘
                             │
                             ▼
                ┌────────────────────────┐
                │   Frontend Trait       │
                │  - eval(code, ctx)     │
                │  - history()           │
                │  - rollback(hash)      │
                └────────────────────────┘
                             │
                             ▼
                ┌────────────────────────┐
                │   TCL Service          │
                │  - TclThreadHandle     │
                │  - State Management    │
                │  - Git Operations      │
                │  - Admin Auth          │
                └────────────────────────┘
                             │
                             ▼
                ┌────────────────────────┐
                │   TCL Interpreter      │
                │  - Safe evaluation     │
                │  - Timeout protection  │
                │  - Sandboxing          │
                └────────────────────────┘
```

## Core Components

### 1. TCL Service (`src/tcl_service.rs`)

Core service that all frontends use:

```rust
pub struct TclService {
    tcl_thread: TclThreadHandle,
    security_config: SecurityConfig,
    tcl_config: TclConfig,
}

pub struct EvalContext {
    pub user: String,
    pub host: String,
    pub channel: Option<String>,
    pub is_admin: bool,
}

pub struct EvalResponse {
    pub output: Vec<String>,  // Lines of output
    pub is_error: bool,
    pub commit_info: Option<CommitInfo>,
    pub more_available: bool,
}

impl TclService {
    pub async fn eval(&mut self, code: &str, ctx: EvalContext) -> Result<EvalResponse>;
    pub async fn history(&self, limit: usize) -> Result<Vec<CommitInfo>>;
    pub async fn rollback(&mut self, commit_hash: &str) -> Result<()>;
    pub async fn more(&mut self, ctx: EvalContext) -> Result<EvalResponse>;
}
```

### 2. Frontend Trait (`src/frontend.rs`)

Common interface for all frontends:

```rust
#[async_trait]
pub trait Frontend {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}

// Each frontend implements this trait
```

### 3. Multi-Frontend Manager (`src/frontend_manager.rs`)

Manages multiple frontends:

```rust
pub struct FrontendManager {
    frontends: Vec<Box<dyn Frontend>>,
    tcl_service: Arc<Mutex<TclService>>,
}

impl FrontendManager {
    pub async fn start_all(&mut self) -> Result<()>;
    pub async fn stop_all(&mut self) -> Result<()>;
}
```

## Frontend Implementations

### 1. IRC Frontend (`src/frontends/irc.rs`)

Refactor existing IRC bot:
- Move IRC-specific logic from `tcl_plugin.rs`
- Implement `Frontend` trait
- Use shared `TclService`
- Keep IRC formatting logic
- PM notifications still work

### 2. CLI Frontend (`src/frontends/cli.rs`)

Interactive command-line REPL:
- Uses `rustyline` for readline functionality
- Command history
- Tab completion for TCL commands
- Syntax highlighting (optional)
- Simple REPL loop

Features:
```
slopdrop> expr {1 + 1}
2
slopdrop> set myvar "hello"
hello
slopdrop> .history
<shows git history>
slopdrop> .rollback <hash>
Rolled back to <hash>
slopdrop> .help
Available commands:
  <tcl code>   - Evaluate TCL
  .history     - Show git history
  .rollback    - Rollback to commit
  .quit        - Exit
```

### 3. TUI Frontend (`src/frontends/tui.rs`)

Full-screen terminal UI using `ratatui`:

Layout:
```
┌─────────────────────────────────────────────────────┐
│ Slopdrop TCL Evalbot                    [Admin]     │
├─────────────────────────────────────────────────────┤
│ Output:                                             │
│ > expr {1 + 1}                                      │
│ 2                                                   │
│ > set myvar "hello"                                 │
│ hello                                               │
│                                                     │
│                                                     │
├─────────────────────────────────────────────────────┤
│ History:                                            │
│ abc1234 - alice - set myvar "hello"                 │
│ def5678 - bob   - proc test {} { ... }              │
├─────────────────────────────────────────────────────┤
│ Input: _                                            │
└─────────────────────────────────────────────────────┘
```

Features:
- Real-time output display
- Scrollable history
- Input area with editing
- Git history sidebar
- Keyboard shortcuts (Ctrl+C to quit, Ctrl+H for history, etc.)

### 4. Web Frontend (`src/frontends/web.rs`)

HTTP/WebSocket server using `axum`:

**REST API:**
```
POST   /api/eval          - Evaluate TCL code
GET    /api/history       - Get commit history
POST   /api/rollback      - Rollback to commit
GET    /api/more          - Get more paginated output
GET    /api/health        - Health check
```

**WebSocket:**
```
WS     /ws                - Real-time updates
```

**Static Files:**
```
GET    /                  - Serve web UI
GET    /static/*          - Static assets
```

**Web UI Features:**
- Monaco editor for TCL code
- Syntax highlighting
- Output display with ANSI colors
- Git history viewer
- Admin controls (rollback, etc.)
- Authentication (optional)

## Configuration

Update `config.toml` to support multiple frontends:

```toml
[frontends]
# Enable/disable frontends
irc = true
cli = false
tui = false
web = true

[frontends.irc]
# Existing IRC config
server = "irc.libera.chat"
# ...

[frontends.cli]
# CLI-specific config
prompt = "slopdrop> "
history_file = ".slopdrop_history"

[frontends.tui]
# TUI-specific config
refresh_rate_ms = 100

[frontends.web]
# Web server config
bind_address = "127.0.0.1"
port = 8080
enable_auth = false
# auth_token = "secret"
```

## Dependencies to Add

```toml
# CLI
rustyline = "13.0"

# TUI
ratatui = "0.26"
crossterm = "0.27"

# Web
axum = "0.7"
tower = "0.4"
tower-http = "0.5"
tokio-tungstenite = "0.21"  # WebSocket

# Shared
async-trait = "0.1"
```

## Implementation Plan

### Phase 1: Core Refactoring
1. ✅ Create `src/tcl_service.rs` - extract core service
2. ✅ Create `src/frontend.rs` - define Frontend trait
3. ✅ Create `src/frontend_manager.rs` - manage multiple frontends
4. ✅ Refactor IRC to use TclService
5. ✅ Update config to support multiple frontends

### Phase 2: CLI Frontend
1. ✅ Create `src/frontends/cli.rs`
2. ✅ Implement rustyline REPL
3. ✅ Add command history
4. ✅ Add special commands (.history, .rollback, .help)
5. ✅ Test CLI frontend

### Phase 3: TUI Frontend
1. ✅ Create `src/frontends/tui.rs`
2. ✅ Implement ratatui layout
3. ✅ Add output display
4. ✅ Add input handling
5. ✅ Add git history view
6. ✅ Test TUI frontend

### Phase 4: Web Frontend
1. ✅ Create `src/frontends/web.rs`
2. ✅ Implement axum REST API
3. ✅ Add WebSocket support
4. ✅ Create web UI (HTML/CSS/JS)
5. ✅ Add authentication (optional)
6. ✅ Test web frontend

### Phase 5: Integration & Testing
1. ✅ Test all frontends together
2. ✅ Update documentation
3. ✅ Add tests for new frontends
4. ✅ Update README with examples

## Security Considerations

### CLI/TUI
- Run locally only
- Inherit admin privileges from user running the process
- No network exposure

### Web
- **Authentication required** for admin operations (rollback, etc.)
- Rate limiting on API endpoints
- CORS configuration
- Optional TLS/HTTPS support
- Session management
- CSRF protection for state-changing operations

## Backward Compatibility

- IRC frontend remains fully functional
- Existing config files still work (with IRC enabled by default)
- New frontends are opt-in

## Benefits

1. **Flexibility**: Use TCL evalbot from multiple interfaces
2. **Development**: Test TCL code locally without IRC
3. **Debugging**: TUI provides better visibility into state
4. **Integration**: Web API allows external tools to use the bot
5. **User Choice**: Different users can use their preferred interface

## Example Usage

### CLI Mode
```bash
$ slopdrop --cli
slopdrop> expr {1 + 1}
2
slopdrop> .quit
```

### TUI Mode
```bash
$ slopdrop --tui
# Full-screen TUI appears
```

### Web Mode
```bash
$ slopdrop --web
Starting web server on http://127.0.0.1:8080
```

### All Frontends
```bash
$ slopdrop --irc --web
IRC client connected to irc.libera.chat:6697
Web server listening on http://127.0.0.1:8080
```

## Future Enhancements

1. **Discord Frontend** - Discord bot interface
2. **Slack Frontend** - Slack bot interface
3. **Matrix Frontend** - Matrix chat interface
4. **gRPC API** - For programmatic access
5. **GraphQL API** - Alternative to REST
6. **Mobile App** - React Native or Flutter app using web API
