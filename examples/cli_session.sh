#!/bin/bash
# Example CLI session demonstrating slopdrop CLI frontend
# Usage: ./examples/cli_session.sh

# Build with CLI frontend support
echo "Building slopdrop with CLI frontend..."
cargo build --release --features frontend-cli

# Start CLI (this will be interactive)
echo "Starting CLI REPL..."
echo "Try these commands:"
echo "  expr {1 + 1}"
echo "  set greeting \"Hello, World!\""
echo "  proc factorial {n} { if {\$n <= 1} { return 1 }; expr {\$n * [factorial [expr {\$n - 1}]]} }"
echo "  factorial 5"
echo "  .history"
echo "  .help"
echo "  .quit"
echo ""

./target/release/slopdrop --cli
