# Frontend User Guide

Complete guide to using all slopdrop frontends: IRC, CLI, TUI, and Web.

## Overview

Slopdrop now supports **4 different frontends** that all use the same TCL backend:

| Frontend | Use Case | Interface | Best For |
|----------|----------|-----------|----------|
| **IRC** | Chat-based bot | IRC channels | Team collaboration, public bots |
| **CLI** | Command-line REPL | Terminal | Quick testing, scripting |
| **TUI** | Full-screen terminal | Terminal | Development, debugging |
| **Web** | Browser + API | HTTP/Browser | Remote access, integration |

All frontends share:
- ✅ Same TCL interpreter and state
- ✅ Git-backed state persistence
- ✅ Output pagination
- ✅ History and rollback
- ✅ Admin privileges

## Building Slopdrop

### Build Specific Frontends

```bash
# IRC only (default)
cargo build --release

# CLI only
cargo build --release --features frontend-cli

# TUI only
cargo build --release --features frontend-tui

# Web only
cargo build --release --features frontend-web

# All frontends
cargo build --release --features all-frontends
```

### Feature Flags

- `default` = `["frontend-irc"]` - IRC only
- `frontend-cli` - Adds CLI REPL (requires: rustyline)
- `frontend-tui` - Adds TUI (requires: ratatui, crossterm)
- `frontend-web` - Adds Web server (requires: axum, tower, tower-http)
- `all-frontends` - Enables all frontends

## 1. IRC Frontend (Default)

The original IRC bot interface - no changes to existing functionality.

### Usage

```bash
# Run with default config
./slopdrop

# Or specify config
./slopdrop config.toml

# Or explicitly use IRC flag
./slopdrop --irc
```

### Configuration

See `config.toml.example` for full IRC configuration.

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
```

### IRC Commands

In channel:
```
tcl expr {1 + 1}              # Evaluate TCL
tcl more                      # Get more paginated output
tclAdmin history              # Show git history (admin only)
tclAdmin rollback <hash>      # Rollback to commit (admin only)
```

## 2. CLI Frontend

Interactive command-line REPL with readline support.

### Usage

```bash
# Start CLI
./slopdrop --cli

# With custom config
./slopdrop config.toml --cli
```

### Features

- ✅ **Readline** - Arrow keys, history navigation
- ✅ **History** - Persistent command history in `.slopdrop_history`
- ✅ **Tab completion** - (future enhancement)
- ✅ **Multi-line input** - Write complex TCL scripts
- ✅ **Special commands** - Built-in commands prefixed with `.`

### CLI Commands

```bash
slopdrop> expr {1 + 1}
2

slopdrop> set myvar "hello world"
hello world

slopdrop> proc greet {name} { return "Hello, $name!" }

slopdrop> greet Alice
Hello, Alice!

# Special commands
slopdrop> .help                  # Show help
slopdrop> .history               # Show last 10 commits
slopdrop> .history 20            # Show last 20 commits
slopdrop> .rollback abc1234      # Rollback to commit
slopdrop> .more                  # Get more paginated output
slopdrop> .quit                  # Exit (or .exit)
```

### Keyboard Shortcuts

- `Ctrl+C` - Exit
- `Ctrl+D` - Exit (EOF)
- `Up/Down` - Navigate history
- `Left/Right` - Move cursor
- `Backspace` - Delete character

### Example Session

```bash
$ slopdrop --cli
Welcome to Slopdrop TCL Evalbot
Type '.help' for help, '.quit' to exit

slopdrop> # Let's create a factorial function
slopdrop> proc factorial {n} {
>     if {$n <= 1} {
>         return 1
>     } else {
>         return [expr {$n * [factorial [expr {$n - 1}]]}]
>     }
> }

slopdrop> factorial 5
120

slopdrop> # Check git history
slopdrop> .history 3
Git History:
  abc1234 - alice - Evaluated proc factorial...
  def5678 - alice - Evaluated set myvar...
  ghi9012 - bob - Evaluated proc greet...

slopdrop> .quit
$
```

## 3. TUI Frontend

Full-screen terminal UI with multiple panes.

### Usage

```bash
# Start TUI
./slopdrop --tui

# With custom config
./slopdrop config.toml --tui
```

### Features

- ✅ **Split-pane layout** - Output, input, history, status
- ✅ **Keyboard-driven** - No mouse required
- ✅ **Real-time updates** - Git history refreshes automatically
- ✅ **Multi-line editor** - Write complex scripts
- ✅ **Scrollable output** - Review previous results

### Layout

```
┌─────────────────────────────────────────────────────┐
│ Slopdrop TCL Evalbot                    [Admin]     │
├─────────────────────────────────────────────────────┤
│ Output:                                             │
│ > expr {1 + 1}                                      │
│ 2                                                   │
│ > set myvar "hello"                                 │
│ hello                                               │
│ [Git] abc1234 | 2 files (+3 -1)                     │
│                                                     │
├─────────────────────────────────────────────────────┤
│ Input: (Ctrl+Enter to eval, Ctrl+C to quit)        │
│ _                                                   │
├─────────────────────────────────────────────────────┤
│ Git History: (F3 to refresh)                        │
│ abc1234 - alice - Evaluated set myvar "hello"       │
│ def5678 - bob   - Evaluated proc test...            │
├─────────────────────────────────────────────────────┤
│ Status: Ready                                       │
└─────────────────────────────────────────────────────┘
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Evaluate current input |
| `Ctrl+C` | Quit |
| `Enter` | New line in input |
| `Backspace` | Delete character |
| `Left/Right` | Move cursor |
| `F2` | Get more paginated output |
| `F3` | Refresh git history |

### Example Session

1. Start TUI: `./slopdrop --tui`
2. Type TCL code in input area
3. Press `Ctrl+Enter` to evaluate
4. See results in output area
5. Git history updates automatically in sidebar
6. Press `F2` if output is truncated
7. Press `Ctrl+C` to quit

## 4. Web Frontend

HTTP REST API with embedded web UI.

### Usage

```bash
# Start web server (default: http://127.0.0.1:8080)
./slopdrop --web

# With custom config
./slopdrop config.toml --web
```

Then open `http://127.0.0.1:8080` in your browser.

### Features

- ✅ **REST API** - JSON endpoints for external tools
- ✅ **Web UI** - Single-page app with Monaco-style editor
- ✅ **Real-time** - Instant feedback
- ✅ **Git history** - Click to rollback
- ✅ **CORS enabled** - Use from any origin
- ✅ **Responsive** - Works on desktop and mobile

### REST API Endpoints

#### POST /api/eval
Evaluate TCL code.

**Request:**
```json
{
  "code": "expr {1 + 1}",
  "user": "alice",
  "is_admin": true
}
```

**Response:**
```json
{
  "output": ["2"],
  "is_error": false,
  "commit_info": null,
  "more_available": false
}
```

#### GET /api/more?user=alice
Get more paginated output.

**Response:**
```json
{
  "output": ["line 11", "line 12", "..."],
  "is_error": false,
  "commit_info": null,
  "more_available": true
}
```

#### GET /api/history
Get git commit history.

**Response:**
```json
[
  {
    "commit_id": "abc1234567890...",
    "author": "alice",
    "message": "Evaluated set myvar...",
    "files_changed": 2,
    "insertions": 3,
    "deletions": 1
  },
  ...
]
```

#### POST /api/rollback
Rollback to a specific commit.

**Request:**
```json
{
  "commit_hash": "abc1234"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Rolled back to commit abc1234. TCL thread restarted with new state."
}
```

#### GET /api/health
Health check.

**Response:**
```json
{
  "success": true,
  "message": "OK"
}
```

### Web UI Features

- **Code Editor** - Textarea with monospace font
- **Syntax Highlighting** - CSS-based highlighting
- **Output Display** - Scrollable output with colors
- **Git History** - Sidebar with commit list
- **Keyboard Shortcuts**:
  - `Ctrl+Enter` - Evaluate code
  - `Ctrl+L` - Clear output
- **Click to Rollback** - Click any commit to rollback

### Using the API from Code

**JavaScript:**
```javascript
// Evaluate TCL code
const response = await fetch('http://localhost:8080/api/eval', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    code: 'expr {1 + 1}',
    is_admin: true
  })
});

const result = await response.json();
console.log(result.output); // ["2"]
```

**Python:**
```python
import requests

# Evaluate TCL code
response = requests.post('http://localhost:8080/api/eval', json={
    'code': 'expr {1 + 1}',
    'is_admin': True
})

result = response.json()
print(result['output'])  # ['2']
```

**curl:**
```bash
# Evaluate TCL code
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

## Running Multiple Frontends

You can run multiple frontends **simultaneously**!

### IRC + Web

Perfect for team collaboration with web access:

```bash
./slopdrop --irc --web
```

- IRC bot runs in channels
- Web UI available for admin/debugging
- Both share same TCL state
- Changes in IRC visible in web UI

### CLI + TUI

Development setup with dual terminals:

```bash
# Terminal 1
./slopdrop --cli

# Terminal 2 (same config)
./slopdrop --tui
```

**Note:** Only one frontend can run at a time with the same state directory.

### All Frontends

```bash
./slopdrop --irc --cli --tui --web
```

**Warning:** CLI and TUI both use stdin, so this combination won't work well. Use IRC + Web or CLI/TUI + Web instead.

## Configuration

### Basic Configuration

All frontends use the same `config.toml`:

```toml
[tcl]
state_path = "./state"
state_repo = "git@github.com:user/repo.git"  # Optional remote
ssh_key = "/home/user/.ssh/id_rsa"          # Optional SSH key
max_output_lines = 10

[security]
eval_timeout_ms = 30000
privileged_users = [
    "alice!*@*.example.com"
]
```

### Frontend-Specific Configuration

*Future enhancement - not yet implemented*

```toml
[frontends.cli]
prompt = "slopdrop> "
history_file = ".slopdrop_history"

[frontends.tui]
refresh_rate_ms = 100

[frontends.web]
bind_address = "127.0.0.1"
port = 8080
enable_auth = false
auth_token = "secret"
```

## Tips & Tricks

### 1. Quick Testing with CLI

Use CLI for quick TCL testing without IRC:

```bash
./slopdrop --cli
slopdrop> expr {1 + 1}
2
slopdrop> .quit
```

### 2. Development with TUI

Use TUI for better visibility during development:

```bash
./slopdrop --tui
# See output, input, and git history all at once
```

### 3. Remote Access with Web

Start web server and access from anywhere:

```bash
./slopdrop --web
# Access from http://your-server:8080
```

### 4. IRC Bot with Web Admin

Run IRC bot with web interface for admin tasks:

```bash
./slopdrop --irc --web
```

- Users interact via IRC
- Admins use web UI for rollback/history

### 5. Scripting with Web API

Automate TCL evaluation from scripts:

```bash
#!/bin/bash
# evaluate.sh
curl -X POST http://localhost:8080/api/eval \
  -H 'Content-Type: application/json' \
  -d "{\"code\":\"$1\",\"is_admin\":true}"
```

Usage: `./evaluate.sh "expr {1 + 1}"`

## Troubleshooting

### CLI: "readline error"

**Problem:** Error creating readline editor

**Solution:** Ensure terminal supports readline features. Try a different terminal or use simpler input.

### TUI: Display issues

**Problem:** UI rendering incorrectly

**Solution:**
- Ensure terminal supports ANSI colors
- Try resizing terminal
- Check TERM environment variable

### Web: Port already in use

**Problem:** "Failed to bind to address"

**Solution:** Change port in configuration or stop other process using port 8080.

### All: "Failed to create TCL service"

**Problem:** State directory issues

**Solution:**
- Check permissions on state directory
- Ensure git is initialized (happens automatically)
- Check disk space

## Best Practices

1. **Use CLI for quick tests** - Fast REPL for development
2. **Use TUI for debugging** - Better visibility into state
3. **Use Web for integration** - API access for external tools
4. **Use IRC for collaboration** - Team-based TCL evaluation
5. **Version control** - All state changes are git-committed
6. **Admin separation** - Use privileged_users for sensitive ops
7. **Pagination** - Large outputs are automatically paginated

## Security Considerations

### CLI/TUI
- Run locally only
- Inherits system user's permissions
- No network exposure
- All users are admins by default

### Web
- **Binds to 127.0.0.1 by default** (local only)
- Change bind_address to allow remote access
- No authentication by default (future enhancement)
- CORS enabled (be careful in production)
- Consider firewall rules for port 8080

### IRC
- Hostmask-based authentication
- Wildcard support in privileged_users
- PM notifications to admins only
- Rate limiting on API calls

## Examples

See `examples/` directory for:
- CLI scripts
- Web API clients (Python, JavaScript, curl)
- TUI session recordings
- IRC bot configuration examples

## Contributing

Want to add more frontends?

1. Implement `Frontend` trait in `src/frontend.rs`
2. Use `TclService` for TCL evaluation
3. Add feature flag to `Cargo.toml`
4. Update `main.rs` to support new frontend
5. Add documentation and examples

Potential frontends:
- Discord bot
- Slack bot
- Matrix bridge
- Telegram bot
- gRPC server
- GraphQL API
- Mobile app

## Support

For issues, questions, or contributions, see:
- README.md
- MULTI_FRONTEND_DESIGN.md
- GitHub issues
