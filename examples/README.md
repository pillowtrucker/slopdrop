# Slopdrop Examples

This directory contains example scripts demonstrating the different frontends and API usage.

## Quick Start

### CLI REPL
```bash
./examples/cli_session.sh
```

Interactive command-line REPL for TCL evaluation.

### TUI Demo
```bash
./examples/tui_demo.sh
```

Full-screen terminal UI demonstration.

### Web API - Python Client
```bash
# Start the web server first
./target/release/slopdrop --web

# In another terminal
python3 examples/web_api_client.py
```

Demonstrates using the REST API from Python.

### Web API - JavaScript Client
```bash
# Start the web server first
./target/release/slopdrop --web

# In another terminal (requires Node.js)
node examples/web_api_client.js
```

Demonstrates using the REST API from JavaScript/Node.js.

### Web API - curl Examples
```bash
# Start the web server first
./target/release/slopdrop --web

# In another terminal
./examples/curl_examples.sh
```

Shows raw HTTP API usage with curl commands.

### Multi-Frontend Demo
```bash
./examples/multi_frontend_demo.sh
```

Runs multiple frontends simultaneously (IRC + Web).

## Example Scripts

| Script | Description | Requirements |
|--------|-------------|--------------|
| `cli_session.sh` | Interactive CLI REPL demo | Cargo, frontend-cli feature |
| `tui_demo.sh` | Full-screen TUI demo | Cargo, frontend-tui feature |
| `web_api_client.py` | Python REST API client | Python 3, requests library |
| `web_api_client.js` | JavaScript REST API client | Node.js (with fetch support) |
| `curl_examples.sh` | curl command examples | curl, jq (optional) |
| `multi_frontend_demo.sh` | Run multiple frontends | Cargo, all-frontends feature |

## Prerequisites

### For CLI/TUI examples:
```bash
cargo build --release --features all-frontends
```

### For Python examples:
```bash
pip3 install requests
```

### For JavaScript examples:
```bash
# Requires Node.js 18+ (has built-in fetch)
node --version  # Should be >= 18.0.0
```

### For curl examples:
```bash
# Ubuntu/Debian
apt-get install curl jq

# macOS
brew install curl jq
```

## Web API Endpoints

All web API examples use these endpoints:

- `GET /api/health` - Health check
- `POST /api/eval` - Evaluate TCL code
- `GET /api/more` - Get more paginated output
- `GET /api/history?limit=N` - Get git commit history
- `POST /api/rollback` - Rollback to specific commit

See FRONTEND_GUIDE.md for complete API documentation.

## Making Examples Executable

```bash
chmod +x examples/*.sh
chmod +x examples/*.py
chmod +x examples/*.js
```

## Tips

1. **Always start the server first** when testing web API examples
2. **Use jq** with curl for pretty JSON formatting
3. **Try multiple frontends** together to see shared state
4. **Check logs** for detailed error messages
5. **Configure properly** - copy config.toml.example to config.toml

## Troubleshooting

### "Cannot connect to server"
Make sure the web frontend is running:
```bash
./target/release/slopdrop --web
```

### "Command not found: slopdrop"
Build the project first:
```bash
cargo build --release --features all-frontends
```

### Python "Module not found: requests"
Install the requests library:
```bash
pip3 install requests
```

### Node.js "fetch is not defined"
Update to Node.js 18 or later, or install node-fetch:
```bash
npm install node-fetch
```

## More Examples

For more detailed examples and usage patterns, see:
- [FRONTEND_GUIDE.md](../FRONTEND_GUIDE.md) - Complete frontend documentation
- [README_NEW.md](../README_NEW.md) - Quick start and overview
- [MULTI_FRONTEND_DESIGN.md](../MULTI_FRONTEND_DESIGN.md) - Architecture details
