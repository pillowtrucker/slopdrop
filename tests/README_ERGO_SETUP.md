# Ergo IRC Server Setup

This directory uses a setup script to download the Ergo IRC server binary instead of storing it in git.

## Why?

The Ergo binary is ~10MB, which is too large to store in the git repository. Instead, we download it on-demand when running tests.

## How it works

1. **setup_ergo.sh** - Downloads Ergo v2.14.0 from GitHub releases if not already present
2. Test scripts automatically call setup_ergo.sh before starting tests
3. The binary is cached in `tests/ergo/ergo` after first download
4. `.gitignore` excludes the binary from git tracking

## Manual setup

If you want to set up Ergo manually before running tests:

```bash
./tests/setup_ergo.sh
```

## Running tests

The test scripts will automatically download Ergo if needed:

```bash
# Integration tests (quick)
./tests/run_integration_tests.sh

# Comprehensive tests (full suite)
./tests/comprehensive_tests.sh
```

## Version

Currently using **Ergo v2.14.0** (linux-x86_64)

Download URL: https://github.com/ergochat/ergo/releases/download/v2.14.0/ergo-2.14.0-linux-x86_64.tar.gz

## Updating Ergo version

To update to a newer version, edit `tests/setup_ergo.sh` and change the `ERGO_VERSION` variable:

```bash
ERGO_VERSION="2.14.0"  # Change this to newer version
```

Then remove the old binary and re-run setup:

```bash
rm tests/ergo/ergo
./tests/setup_ergo.sh
```
