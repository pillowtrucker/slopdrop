#!/bin/bash
# Demonstration of running multiple frontends simultaneously

echo "=== Multi-Frontend Demo ==="
echo ""
echo "This demo shows how to run multiple slopdrop frontends at once"
echo "All frontends share the same TCL interpreter state!"
echo ""

# Build with all frontends
echo "Building with all frontends..."
cargo build --release --features all-frontends

echo ""
echo "Starting IRC + Web frontends..."
echo "  - IRC bot will connect to configured server"
echo "  - Web UI available at http://127.0.0.1:8080"
echo ""
echo "You can:"
echo "  1. Evaluate TCL in IRC: 'tcl expr {1 + 1}'"
echo "  2. See the same state in web UI at http://127.0.0.1:8080"
echo "  3. Define a proc in web UI"
echo "  4. Call it from IRC"
echo ""
echo "Press Ctrl+C to stop all frontends"
echo ""

# Run with both IRC and Web
./target/release/slopdrop --irc --web
