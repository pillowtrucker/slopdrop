# Slopdrop Integration Tests

This directory contains integration tests for the slopdrop IRC TCL evalbot.

## Overview

The integration tests use:
- **Ergo IRC Server** - A real IRC server for testing
- **Bash test harness** - Simple shell script to orchestrate tests
- **Netcat** - To simulate IRC clients sending commands

## Running Tests

```bash
./tests/run_integration_tests.sh
```

This will:
1. Build the slopdrop bot (release mode)
2. Start Ergo IRC server on port 16667
3. Start the bot and have it join #test
4. Run test scenarios with netcat-based IRC clients
5. Verify bot responses
6. Clean up (stop bot and IRC server)

## Test Coverage

### Test 1: Basic Connection
Verifies that the bot can connect to IRC server and is running properly.

### Test 2: TCL Evaluation
- Sends: `tcl expr {2 + 2}`
- Expected: Bot responds with `4`
- Validates: Core TCL evaluation works

### Test 3: IRC Formatting Strip
- Sends: `tcl expr {3 + 3}` with IRC color codes (\x03)
- Expected: Bot responds with `6`
- Validates: Color codes are stripped from input before processing

### Test 4: Channel Member Tracking
- Sends: `tcl chanlist #test`
- Expected: Bot responds with list of nicknames in channel
- Validates: Channel tracking and chanlist command work

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
