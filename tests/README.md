# Slopdrop Integration Tests

This directory contains comprehensive integration tests for the slopdrop IRC TCL evalbot.

## Overview

The integration tests use:
- **Ergo IRC Server v2.14.0** - A real IRC server for testing (not mocks!)
- **Bash test harness** - Shell scripts to orchestrate tests
- **Netcat** - To simulate IRC clients sending commands

## Test Suites

### 1. Basic Integration Tests (`run_integration_tests.sh`)
Quick smoke tests (4 tests, ~15 seconds):
```bash
./tests/run_integration_tests.sh
```

Tests:
- Bot connection
- Basic TCL evaluation
- IRC formatting strip
- Channel tracking

### 2. Comprehensive Test Suite (`comprehensive_tests.sh`) ⭐
**28 comprehensive tests covering ALL functionality** (~90 seconds):
```bash
./tests/comprehensive_tests.sh
```

This will:
1. Build the slopdrop bot (release mode)
2. Start Ergo IRC server on port 16667
3. Start the bot and have it join #test
4. Run test scenarios with netcat-based IRC clients
5. Verify bot responses
6. Clean up (stop bot and IRC server)

## Comprehensive Test Coverage (28 tests)

**All 28 tests passing! ✓✓✓**

### Section 1: Basic TCL Evaluation (4 tests)
- Basic math operations (2+2=4, 10*5=50)
- String operations (length)
- List operations

### Section 2: State Persistence (3 tests)
- Proc definition and calling across sessions
- Variable persistence across sessions
- Git state tracking

### Section 3: Cache Commands (4 tests)
- cache::put and cache::get
- cache::keys
- cache::exists (before/after delete)
- cache::delete

### Section 4: Encoding Commands (3 tests)
- Base64 encode/decode
- URL encoding

### Section 5: SHA1 Hashing (1 test)
- SHA1 command (if tcllib available)

### Section 6: Channel Tracking (1 test)
- chanlist command returns member list

### Section 7: History (1 test)
- history command responds with git log

### Section 8: Error Handling (3 tests)
- Division by zero errors
- Undefined command errors
- Syntax errors

### Section 9: IRC Formatting (2 tests)
- Color code stripping (\x03)
- Bold code stripping (\x02)

### Section 10: Privilege System (2 tests)
- Admin commands denied for unprivileged users
- Admin commands allowed for privileged users (testuser)

### Section 11: Multi-line Output (1 test)
- Multiple PRIVMSG lines sent correctly

### Section 12: Output Truncation (1 test)
- Long output truncated at max_output_lines (10)

### Section 13: Context Variables (2 tests)
- ::nick variable set correctly
- ::channel variable set correctly

## Test Files

- `run_integration_tests.sh` - Main test harness (bash)
- `test_config.toml` - Bot configuration for tests
- `ergo/` - Ergo IRC server binary and configuration
  - `test-ircd.yaml` - Minimal Ergo config for testing
  - `test-motd.txt` - MOTD file

## Exit Codes

- `0` - All tests passed
- `1` - One or more tests failed

## Requirements

- `cargo` - To build the bot
- `nc` (netcat) - To simulate IRC clients
- Port 16667 available for IRC server

## Test Output

Test results are printed to stdout/stderr:
- `✓ PASSED` - Test succeeded
- `✗ FAILED` - Test failed
- `⚠` - Warning/partial success

Detailed IRC protocol logs are written to `/tmp/test_*.log`

## Notes

- Tests use port 16667 to avoid conflicts with production IRC
- Test state is stored in `/tmp/slopdrop_test_state` (cleaned up each run)
- Ergo database is stored in `/tmp/ergo-test.db` (cleaned up each run)
- HTTP commands require tcllib to be installed (tests still pass without it)
- Tests run in ~15 seconds

## Troubleshooting

### IRC server fails to start
Check that port 16667 is not in use:
```bash
lsof -i:16667
```

### Bot fails to connect
Check logs in the test output for error messages. The bot logs are visible in the test output.

### Tests hang
The test script has built-in cleanup on Ctrl+C. Press Ctrl+C to stop.

### HTTP commands not available
Install tcllib:
```bash
sudo apt-get install tcllib
```
Tests will still pass; HTTP commands just won't be available.
