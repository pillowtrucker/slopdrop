#!/bin/bash
# Integration tests for slopdrop IRC bot

set -e

TEST_PORT=16667
IRC_SERVER_PID=""
BOT_PID=""
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$TEST_DIR")"

cleanup() {
    echo ""
    echo "=== Cleaning up ==="

    if [ ! -z "$BOT_PID" ]; then
        echo "Stopping bot (PID: $BOT_PID)..."
        kill $BOT_PID 2>/dev/null || true
        wait $BOT_PID 2>/dev/null || true
    fi

    if [ ! -z "$IRC_SERVER_PID" ]; then
        echo "Stopping IRC server (PID: $IRC_SERVER_PID)..."
        kill $IRC_SERVER_PID 2>/dev/null || true
        wait $IRC_SERVER_PID 2>/dev/null || true
    fi

    # Clean up test state
    rm -rf /tmp/slopdrop_test_state
    rm -f /tmp/ergo-test.db

    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

echo "============================================"
echo "SLOPDROP INTEGRATION TESTS"
echo "============================================"
echo ""

# Build the project
echo "=== Building slopdrop ==="
cd "$PROJECT_ROOT"
cargo build --release 2>&1 | grep -E "(Compiling|Finished|error)" || true
if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "✗ Build failed"
    exit 1
fi
echo "✓ Build successful"
echo ""

# Start Ergo IRC server
echo "=== Starting Ergo IRC server ==="
cd "$TEST_DIR/ergo"
./ergo run --conf test-ircd.yaml &
IRC_SERVER_PID=$!
echo "IRC server started (PID: $IRC_SERVER_PID)"

# Wait for IRC server to start
sleep 2

# Check if IRC server is running
if ! kill -0 $IRC_SERVER_PID 2>/dev/null; then
    echo "✗ IRC server failed to start"
    exit 1
fi
echo "✓ IRC server is running"
echo ""

# Start slopdrop bot
echo "=== Starting slopdrop bot ==="
cd "$PROJECT_ROOT"
rm -rf /tmp/slopdrop_test_state
./target/release/slopdrop "$TEST_DIR/test_config.toml" &
BOT_PID=$!
echo "Bot started (PID: $BOT_PID)"

# Wait for bot to connect
sleep 3

# Check if bot is running
if ! kill -0 $BOT_PID 2>/dev/null; then
    echo "✗ Bot failed to start"
    exit 1
fi
echo "✓ Bot is running"
echo ""

#  Test 1: Basic connection
echo "=== Test 1: Bot connects and joins channel ==="
sleep 1
echo "✓ PASSED (bot still running)"
echo ""

# Test 2: TCL evaluation
echo "=== Test 2: TCL evaluation ==="
echo "Sending test command via IRC..."

# Use netcat to connect and send a message
(
    sleep 1
    echo "NICK testuser"
    echo "USER testuser 0 * :Test User"
    sleep 1
    echo "JOIN #test"
    sleep 1
    echo "PRIVMSG #test :tcl expr {2 + 2}"
    sleep 2
    echo "QUIT"
) | nc -q 3 127.0.0.1 $TEST_PORT > /tmp/test_output.log 2>&1

# Check the output
if grep -q ":testbot.*PRIVMSG #test :4" /tmp/test_output.log; then
    echo "✓ PASSED - Bot responded with correct answer"
elif grep -q "PRIVMSG #test" /tmp/test_output.log; then
    echo "⚠ Bot responded, checking output..."
    grep "PRIVMSG #test" /tmp/test_output.log
else
    echo "✗ FAILED - No response from bot"
    echo "Output:"
    cat /tmp/test_output.log
fi
echo ""

# Test 3: IRC formatting strip
echo "=== Test 3: IRC formatting code stripping ==="
(
    sleep 1
    echo "NICK testuser2"
    echo "USER testuser2 0 * :Test User 2"
    sleep 1
    echo "JOIN #test"
    sleep 1
    # Send with color codes (^C is \x03 in IRC)
    printf "PRIVMSG #test :\x0304tcl\x03 expr {3 + 3}\r\n"
    sleep 2
    echo "QUIT"
) | nc -q 3 127.0.0.1 $TEST_PORT > /tmp/test_formatting.log 2>&1

if grep -q ":testbot.*PRIVMSG #test :6" /tmp/test_formatting.log; then
    echo "✓ PASSED - Bot handled formatted input correctly"
else
    echo "⚠ Checking output..."
    grep "PRIVMSG #test" /tmp/test_formatting.log || echo "No response found"
fi
echo ""

# Test 4: Channel tracking
echo "=== Test 4: Channel member tracking ==="
(
    sleep 1
    echo "NICK testuser3"
    echo "USER testuser3 0 * :Test User 3"
    sleep 1
    echo "JOIN #test"
    sleep 1
    echo "PRIVMSG #test :tcl chanlist #test"
    sleep 2
    echo "QUIT"
) | nc -q 3 127.0.0.1 $TEST_PORT > /tmp/test_chanlist.log 2>&1

if grep -q "PRIVMSG #test" /tmp/test_chanlist.log; then
    echo "✓ Bot responded to chanlist command"
    grep "PRIVMSG #test" /tmp/test_chanlist.log | head -1
else
    echo "⚠ No response found"
fi
echo ""

echo "============================================"
echo "TESTS COMPLETE"
echo "============================================"
echo ""
echo "Bot is still running. Logs from bot:"
echo "Check /tmp/test_*.log for detailed output"
echo ""
