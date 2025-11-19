#!/bin/bash
# Run the Slopdrop container securely
# Usage: ./container-run.sh [options]
#
# Options:
#   --web           Enable web frontend on port 3000
#   --detach        Run in background (default)
#   --foreground    Run in foreground (useful for debugging)
#   --name NAME     Container name (default: slopdrop)
#   --config PATH   Path to config file (default: ./config.toml)
#   --state PATH    Path to state directory (default: ./state)

set -e

# Default values
IMAGE_NAME="slopdrop:latest"
CONTAINER_NAME="slopdrop"
CONFIG_PATH="./config.toml"
STATE_PATH="./state"
DETACH="-d"
WEB_PORT=""
EXTRA_ARGS=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --web)
            WEB_PORT="-p 3000:3000"
            EXTRA_ARGS="--irc --web"
            shift
            ;;
        --detach)
            DETACH="-d"
            shift
            ;;
        --foreground)
            DETACH="-it"
            shift
            ;;
        --name)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        --config)
            CONFIG_PATH="$2"
            shift 2
            ;;
        --state)
            STATE_PATH="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Validate config file exists
if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "Error: Configuration file not found: $CONFIG_PATH"
    echo ""
    echo "Create a config file from the example:"
    echo "  cp config.toml.example config.toml"
    echo "  # Edit config.toml with your settings"
    exit 1
fi

# Create state directory if it doesn't exist
mkdir -p "$STATE_PATH"

# Stop and remove existing container with same name
if podman container exists "$CONTAINER_NAME" 2>/dev/null; then
    echo "Stopping existing container: $CONTAINER_NAME"
    podman stop "$CONTAINER_NAME" 2>/dev/null || true
    podman rm "$CONTAINER_NAME" 2>/dev/null || true
fi

echo "Starting Slopdrop container..."
echo "  Container: $CONTAINER_NAME"
echo "  Config: $CONFIG_PATH"
echo "  State: $STATE_PATH"

# Run the container with security options
podman run $DETACH \
    --name "$CONTAINER_NAME" \
    -v "$(realpath "$CONFIG_PATH")":/app/config/config.toml:ro,Z \
    -v "$(realpath "$STATE_PATH")":/app/state:Z \
    $WEB_PORT \
    --cap-drop=ALL \
    --security-opt=no-new-privileges:true \
    --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,size=64m \
    --memory=512m \
    --memory-swap=512m \
    --cpus=1 \
    --pids-limit=100 \
    --network=slirp4netns \
    "$IMAGE_NAME" \
    /app/config/config.toml $EXTRA_ARGS

if [[ "$DETACH" == "-d" ]]; then
    echo ""
    echo "Container started in background."
    echo ""
    echo "Useful commands:"
    echo "  podman logs -f $CONTAINER_NAME     # View logs"
    echo "  podman stop $CONTAINER_NAME        # Stop container"
    echo "  podman rm $CONTAINER_NAME          # Remove container"
    echo ""
    if [[ -n "$WEB_PORT" ]]; then
        echo "Web frontend available at: http://localhost:3000"
    fi
fi
