#!/bin/bash
# Demo script for slopdrop TUI frontend
# This shows what you can do in the TUI

echo "=== Slopdrop TUI Frontend Demo ==="
echo ""
echo "Building with TUI support..."
cargo build --release --features frontend-tui

echo ""
echo "Starting TUI..."
echo ""
echo "In the TUI you can:"
echo "  • Type TCL code in the input pane"
echo "  • Press Ctrl+Enter to evaluate"
echo "  • Press F2 to get more paginated output"
echo "  • Press F3 to refresh git history"
echo "  • Press Ctrl+C to quit"
echo ""
echo "Try these examples:"
echo "  1. expr {1 + 1}"
echo "  2. set x 10; set y 20; expr {\$x + \$y}"
echo "  3. for {set i 0} {\$i < 10} {incr i} { puts \"Count: \$i\" }"
echo "  4. proc greet {name} { return \"Hello, \$name!\" }; greet \"TUI User\""
echo ""
echo "Press Enter to start TUI..."
read

./target/release/slopdrop --tui
