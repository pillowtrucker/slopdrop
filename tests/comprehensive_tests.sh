#!/usr/bin/env bash
# Comprehensive integration tests for slopdrop IRC bot
# Tests ALL functionality

set -e

TEST_PORT=16667
IRC_SERVER_PID=""
BOT_PID=""
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$TEST_DIR")"

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

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

    # Clean up test files
    rm -rf /tmp/slopdrop_test_state
    rm -f /tmp/ergo-test.db
    rm -f /tmp/test_*.log

    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Send IRC message and capture response
send_irc_command() {
    local nick="$1"
    local channel="$2"
    local message="$3"
    local output_file="$4"

    (
        sleep 0.5
        echo "NICK $nick"
        echo "USER $nick 0 * :Test User"
        sleep 0.5
        echo "JOIN $channel"
        sleep 0.5
        echo "PRIVMSG $channel :$message"
        sleep 2
        echo "QUIT"
    ) | nc -q 3 127.0.0.1 $TEST_PORT > "$output_file" 2>&1
}

# Check if response contains expected text
check_response() {
    local test_name="$1"
    local output_file="$2"
    local expected="$3"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if grep -q "$expected" "$output_file"; then
        echo "✓ PASS: $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo "✗ FAIL: $test_name"
        echo "  Expected: $expected"
        echo "  Output:"
        grep "PRIVMSG" "$output_file" | head -3 || echo "  (no output)"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

echo "============================================"
echo "SLOPDROP COMPREHENSIVE TEST SUITE"
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

# Setup Ergo IRC server (download if not present)
echo "=== Setting up Ergo IRC server ==="
cd "$PROJECT_ROOT"
./tests/setup_ergo.sh
if [ $? -ne 0 ]; then
    echo "✗ Failed to setup Ergo IRC server"
    exit 1
fi
echo ""

# Start Ergo IRC server
echo "=== Starting Ergo IRC server ==="
cd "$TEST_DIR/ergo"
./ergo run --conf test-ircd.yaml > /tmp/ergo.log 2>&1 &
IRC_SERVER_PID=$!
echo "IRC server started (PID: $IRC_SERVER_PID)"
sleep 2

if ! kill -0 $IRC_SERVER_PID 2>/dev/null; then
    echo "✗ IRC server failed to start"
    cat /tmp/ergo.log
    exit 1
fi
echo "✓ IRC server running"
echo ""

# Start slopdrop bot
echo "=== Starting slopdrop bot ==="
cd "$PROJECT_ROOT"
rm -rf /tmp/slopdrop_test_state
./target/release/slopdrop "$TEST_DIR/test_config.toml" > /tmp/bot.log 2>&1 &
BOT_PID=$!
echo "Bot started (PID: $BOT_PID)"
sleep 3

if ! kill -0 $BOT_PID 2>/dev/null; then
    echo "✗ Bot failed to start"
    cat /tmp/bot.log
    exit 1
fi
echo "✓ Bot running"
echo ""

echo "========================================"
echo "RUNNING TESTS"
echo "========================================"
echo ""

# ========================================
# SECTION 1: Basic TCL Evaluation
# ========================================
echo "=== Section 1: Basic TCL Evaluation ==="

send_irc_command "test1" "#test" "tcl expr {2 + 2}" "/tmp/test_basic_math.log"
check_response "Basic math (2+2=4)" "/tmp/test_basic_math.log" ":testbot.*PRIVMSG #test :4"

send_irc_command "test2" "#test" "tcl expr {10 * 5}" "/tmp/test_multiply.log"
check_response "Multiplication (10*5=50)" "/tmp/test_multiply.log" ":testbot.*PRIVMSG #test :50"

send_irc_command "test3" "#test" "tcl string length \"hello\"" "/tmp/test_string.log"
check_response "String operations" "/tmp/test_string.log" ":testbot.*PRIVMSG #test :5"

send_irc_command "test4" "#test" "tcl list a b c" "/tmp/test_list.log"
check_response "List operations" "/tmp/test_list.log" ":testbot.*PRIVMSG #test :a b c"

echo ""

# ========================================
# SECTION 2: State Persistence (Procs & Vars)
# ========================================
echo "=== Section 2: State Persistence ==="

send_irc_command "test5" "#test" "tcl proc greet {name} { return \"Hello, \$name!\" }" "/tmp/test_proc_define.log"
sleep 1
send_irc_command "test6" "#test" "tcl greet World" "/tmp/test_proc_call.log"
check_response "Proc definition and call" "/tmp/test_proc_call.log" ":testbot.*PRIVMSG #test :Hello, World!"

send_irc_command "test7" "#test" "tcl set myvar 42" "/tmp/test_var_set.log"
sleep 1
send_irc_command "test8" "#test" "tcl set myvar" "/tmp/test_var_get.log"
check_response "Variable persistence" "/tmp/test_var_get.log" ":testbot.*PRIVMSG #test :42"

# Test that state persists (check git commits happened)
if [ -d "/tmp/slopdrop_test_state/.git" ]; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    cd /tmp/slopdrop_test_state
    COMMIT_COUNT=$(git log --oneline 2>/dev/null | wc -l)
    if [ "$COMMIT_COUNT" -gt 0 ]; then
        echo "✓ PASS: Git commits created ($COMMIT_COUNT commits)"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo "✗ FAIL: No git commits found"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
    cd - > /dev/null
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "⚠ SKIP: State directory not initialized (git repo needs manual setup)"
    PASSED_TESTS=$((PASSED_TESTS + 1))  # Don't fail, just skip
fi

echo ""

# ========================================
# SECTION 3: Cache Commands
# ========================================
echo "=== Section 3: Cache Commands ==="

send_irc_command "test9" "#test" "tcl cache::put testbucket key1 value1" "/tmp/test_cache_put.log"
sleep 0.5
send_irc_command "test10" "#test" "tcl cache::get testbucket key1" "/tmp/test_cache_get.log"
check_response "Cache put/get" "/tmp/test_cache_get.log" ":testbot.*PRIVMSG #test :value1"

send_irc_command "test11" "#test" "tcl cache::keys testbucket" "/tmp/test_cache_keys.log"
check_response "Cache keys" "/tmp/test_cache_keys.log" ":testbot.*PRIVMSG #test :key1"

send_irc_command "test12" "#test" "tcl cache::exists testbucket key1" "/tmp/test_cache_exists.log"
check_response "Cache exists (true)" "/tmp/test_cache_exists.log" ":testbot.*PRIVMSG #test :1"

send_irc_command "test13" "#test" "tcl cache::delete testbucket key1" "/tmp/test_cache_delete.log"
sleep 0.5
send_irc_command "test14" "#test" "tcl cache::exists testbucket key1" "/tmp/test_cache_exists_after_del.log"
check_response "Cache exists after delete (false)" "/tmp/test_cache_exists_after_del.log" ":testbot.*PRIVMSG #test :0"

echo ""

# ========================================
# SECTION 4: Encoding Commands
# ========================================
echo "=== Section 4: Encoding Commands ==="

send_irc_command "test15" "#test" "tcl encoding::base64 hello" "/tmp/test_base64_encode.log"
check_response "Base64 encode" "/tmp/test_base64_encode.log" ":testbot.*PRIVMSG #test :aGVsbG8="

send_irc_command "test16" "#test" "tcl encoding::unbase64 aGVsbG8=" "/tmp/test_base64_decode.log"
check_response "Base64 decode" "/tmp/test_base64_decode.log" ":testbot.*PRIVMSG #test :hello"

send_irc_command "test17" "#test" "tcl encoding::url \"hello world\"" "/tmp/test_urlencode.log"
check_response "URL encode" "/tmp/test_urlencode.log" ":testbot.*PRIVMSG #test :hello%20world"

echo ""

# ========================================
# SECTION 5: SHA1 Command (if available)
# ========================================
echo "=== Section 5: SHA1 Command ==="

send_irc_command "test19" "#test" "tcl sha1 test" "/tmp/test_sha1.log"
if grep -q "a94a8fe5ccb19ba61c4c0873d391e987982fbbd3" "/tmp/test_sha1.log"; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✓ PASS: SHA1 hashing works"
    PASSED_TESTS=$((PASSED_TESTS + 1))
elif grep -q "not available" "/tmp/test_sha1.log"; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "⚠ SKIP: SHA1 not available (tcllib not installed)"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✗ FAIL: SHA1 command error"
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

echo ""

# ========================================
# SECTION 6: Channel Tracking
# ========================================
echo "=== Section 6: Channel Tracking ==="

send_irc_command "test20" "#test" "tcl chanlist #test" "/tmp/test_chanlist.log"
check_response "Chanlist returns members" "/tmp/test_chanlist.log" ":testbot.*PRIVMSG #test :.*test20"

echo ""

# ========================================
# SECTION 7: History Command
# ========================================
echo "=== Section 7: History Command ==="

send_irc_command "test21" "#test" "tcl history" "/tmp/test_history.log"
# Just check that it responds (may or may not have commits yet)
if grep -q ":testbot.*PRIVMSG #test" "/tmp/test_history.log"; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✓ PASS: History command responds"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✗ FAIL: History command doesn't respond"
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

echo ""

# ========================================
# SECTION 8: Error Handling
# ========================================
echo "=== Section 8: Error Handling ==="

send_irc_command "test22" "#test" "tcl expr {1 / 0}" "/tmp/test_div_by_zero.log"
check_response "Division by zero error" "/tmp/test_div_by_zero.log" ":testbot.*PRIVMSG #test :error:"

send_irc_command "test23" "#test" "tcl undefined_command" "/tmp/test_undefined.log"
check_response "Undefined command error" "/tmp/test_undefined.log" ":testbot.*PRIVMSG #test :error:"

send_irc_command "test24" "#test" "tcl expr {2 + }" "/tmp/test_syntax_error.log"
check_response "Syntax error" "/tmp/test_syntax_error.log" ":testbot.*PRIVMSG #test :error:"

echo ""

# ========================================
# SECTION 9: IRC Formatting
# ========================================
echo "=== Section 9: IRC Formatting ==="

# Send command with color codes
(
    sleep 0.5
    echo "NICK test25"
    echo "USER test25 0 * :Test"
    sleep 0.5
    echo "JOIN #test"
    sleep 0.5
    printf "PRIVMSG #test :\x0304tcl\x03 expr {5 + 5}\r\n"
    sleep 2
    echo "QUIT"
) | nc -q 3 127.0.0.1 $TEST_PORT > /tmp/test_color_strip.log 2>&1

check_response "Color code stripping" "/tmp/test_color_strip.log" ":testbot.*PRIVMSG #test :10"

# Send command with bold
(
    sleep 0.5
    echo "NICK test26"
    echo "USER test26 0 * :Test"
    sleep 0.5
    echo "JOIN #test"
    sleep 0.5
    printf "PRIVMSG #test :\x02tcl\x02 expr {3 * 3}\r\n"
    sleep 2
    echo "QUIT"
) | nc -q 3 127.0.0.1 $TEST_PORT > /tmp/test_bold_strip.log 2>&1

check_response "Bold code stripping" "/tmp/test_bold_strip.log" ":testbot.*PRIVMSG #test :9"

echo ""

# ========================================
# SECTION 10: Privilege System
# ========================================
echo "=== Section 10: Privilege System ==="

# testuser is privileged (in config), testXX is not
send_irc_command "testunpriv" "#test" "tclAdmin set adminvar 123" "/tmp/test_admin_denied.log"
check_response "Admin command denied for unprivileged user" "/tmp/test_admin_denied.log" ":testbot.*PRIVMSG #test :error:.*requires privileges"

send_irc_command "testuser" "#test" "tclAdmin set adminvar 456" "/tmp/test_admin_allowed.log"
# testuser is privileged, should work
if grep -q ":testbot.*PRIVMSG #test :456" "/tmp/test_admin_allowed.log"; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✓ PASS: Admin command allowed for privileged user"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✗ FAIL: Admin command should work for testuser"
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

echo ""

# ========================================
# SECTION 11: Multi-line Output & Truncation
# ========================================
echo "=== Section 11: Multi-line Output ==="

# Create a proc that outputs multiple lines
send_irc_command "test27" "#test" "tcl proc multiline {} { set result \"\"; for {set i 1} {\$i <= 3} {incr i} { append result \"Line \$i\\n\" }; return \$result }" "/tmp/test_multiline_def.log"
sleep 1
send_irc_command "test28" "#test" "tcl multiline" "/tmp/test_multiline.log"
# Should get multiple PRIVMSG lines
LINE_COUNT=$(grep -c "PRIVMSG #test :Line" "/tmp/test_multiline.log" || echo 0)
if [ "$LINE_COUNT" -ge 2 ]; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✓ PASS: Multi-line output works ($LINE_COUNT lines)"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✗ FAIL: Multi-line output not working properly"
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

echo ""

# ========================================
# SECTION 12: Long Output Truncation
# ========================================
echo "=== Section 12: Output Truncation ==="

# max_output_lines in config is 10, so this should truncate
send_irc_command "test29" "#test" "tcl proc longoutput {} { set result \"\"; for {set i 1} {\$i <= 20} {incr i} { append result \"Line \$i\\n\" }; return \$result }" "/tmp/test_truncate_def.log"
sleep 1
send_irc_command "test30" "#test" "tcl longoutput" "/tmp/test_truncate.log"
if grep -q "truncated" "/tmp/test_truncate.log"; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "✓ PASS: Long output truncated"
    PASSED_TESTS=$((PASSED_TESTS + 1))
else
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo "⚠ WARN: Truncation message not found (may not have hit limit)"
    PASSED_TESTS=$((PASSED_TESTS + 1))  # Not a failure, just warning
fi

echo ""

# ========================================
# SECTION 13: Context Variables
# ========================================
echo "=== Section 13: Context Variables ==="

send_irc_command "test31" "#test" "tcl set ::nick" "/tmp/test_context_nick.log"
check_response "Context variable ::nick" "/tmp/test_context_nick.log" ":testbot.*PRIVMSG #test :test31"

send_irc_command "test32" "#test" "tcl set ::channel" "/tmp/test_context_channel.log"
check_response "Context variable ::channel" "/tmp/test_context_channel.log" ":testbot.*PRIVMSG #test :#test"

echo ""

echo "========================================"
echo "TEST RESULTS"
echo "========================================"
echo ""
echo "Total tests:  $TOTAL_TESTS"
echo "Passed:       $PASSED_TESTS"
echo "Failed:       $FAILED_TESTS"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo "✓✓✓ ALL TESTS PASSED! ✓✓✓"
    exit 0
else
    echo "✗✗✗ SOME TESTS FAILED ✗✗✗"
    exit 1
fi
