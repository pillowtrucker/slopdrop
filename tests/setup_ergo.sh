#!/usr/bin/env bash
# Setup script to download Ergo IRC server for testing
# This avoids storing the binary in git

set -e

ERGO_VERSION="2.14.0"
ERGO_DIR="tests/ergo"
ERGO_BINARY="$ERGO_DIR/ergo"
ERGO_URL="https://github.com/ergochat/ergo/releases/download/v${ERGO_VERSION}/ergo-${ERGO_VERSION}-linux-x86_64.tar.gz"

# Check if ergo binary already exists
if [ -f "$ERGO_BINARY" ]; then
    echo "✓ Ergo IRC server already installed at $ERGO_BINARY"
    exit 0
fi

echo "=== Setting up Ergo IRC Server v${ERGO_VERSION} ==="

# Create directory if it doesn't exist
mkdir -p "$ERGO_DIR"

# Download ergo
echo "Downloading Ergo from GitHub releases..."
TMP_DIR=$(mktemp -d)

if ! wget -q "$ERGO_URL" -O "$TMP_DIR/ergo.tar.gz"; then
    echo "Error: Failed to download Ergo from $ERGO_URL"
    rm -rf "$TMP_DIR"
    exit 1
fi

# Extract the binary
echo "Extracting Ergo binary..."
tar xzf "$TMP_DIR/ergo.tar.gz" -C "$TMP_DIR"

# Find and move the ergo binary
ERGO_EXTRACTED=$(find "$TMP_DIR" -name "ergo" -type f | head -1)
if [ -z "$ERGO_EXTRACTED" ]; then
    echo "Error: Could not find ergo binary in extracted archive"
    rm -rf "$TMP_DIR"
    exit 1
fi

mv "$ERGO_EXTRACTED" "$ERGO_BINARY"
chmod +x "$ERGO_BINARY"

# Cleanup
rm -rf "$TMP_DIR"

echo "✓ Ergo IRC server installed successfully at $ERGO_BINARY"
echo "✓ Version: $($ERGO_BINARY version)"
