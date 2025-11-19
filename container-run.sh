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
#   --ssh-key PATH  Path to SSH private key for git push (optional)
#   --known-hosts PATH  Path to known_hosts file (optional)
#   --memory SIZE   Container memory limit (default: 512m)
#   --reset-state REF   Reset state to git ref before starting (commit/tag/branch)

set -e

# Default values
IMAGE_NAME="slopdrop:latest"
CONTAINER_NAME="slopdrop"
CONFIG_PATH="./config.toml"
STATE_PATH="./state"
DETACH="-d"
WEB_PORT=""
EXTRA_ARGS=""
SSH_KEY=""
KNOWN_HOSTS=""
MEMORY_LIMIT="512m"
RESET_STATE=""

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
        --ssh-key)
            SSH_KEY="$2"
            shift 2
            ;;
        --known-hosts)
            KNOWN_HOSTS="$2"
            shift 2
            ;;
        --memory)
            MEMORY_LIMIT="$2"
            shift 2
            ;;
        --reset-state)
            RESET_STATE="$2"
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

# Reset state to specified git ref if requested
if [[ -n "$RESET_STATE" ]]; then
    if [[ ! -d "$STATE_PATH/.git" ]]; then
        echo "Error: State directory is not a git repository: $STATE_PATH"
        echo "Cannot reset to ref: $RESET_STATE"
        exit 1
    fi
    echo "Resetting state to: $RESET_STATE"
    (
        cd "$STATE_PATH"
        git fetch --all 2>/dev/null || true
        if ! git rev-parse --verify "$RESET_STATE" >/dev/null 2>&1; then
            echo "Error: Invalid git ref: $RESET_STATE"
            echo "Available refs:"
            git log --oneline -10
            exit 1
        fi

        # Create backup tag of current HEAD before resetting
        BACKUP_TAG="backup-$(date +%Y%m%d-%H%M%S)"
        git tag "$BACKUP_TAG"
        echo "Created backup tag: $BACKUP_TAG"

        # Checkout target state into working directory (preserves history)
        git checkout "$RESET_STATE" -- .

        # Clean untracked files that may have been in old state
        git clean -fd

        # Commit as new history entry (allows future pushes)
        git add -A
        if ! git diff --cached --quiet; then
            git commit -m "Reset state to $RESET_STATE (backup: $BACKUP_TAG)"
            echo "State reset to: $(git log --oneline -1)"
        else
            echo "State already at $RESET_STATE, no changes needed"
        fi
    )
fi

# Build SSH mount arguments
SSH_MOUNTS=""
if [[ -n "$SSH_KEY" ]]; then
    if [[ ! -f "$SSH_KEY" ]]; then
        echo "Error: SSH key not found: $SSH_KEY"
        exit 1
    fi
    SSH_MOUNTS="$SSH_MOUNTS -v $(realpath "$SSH_KEY"):/app/.ssh/id_rsa:ro,Z"
    echo "Note: SSH key will be mounted at /app/.ssh/id_rsa"
    echo "      Make sure your config.toml has: ssh_key = \"/app/.ssh/id_rsa\""
fi

if [[ -n "$KNOWN_HOSTS" ]]; then
    if [[ ! -f "$KNOWN_HOSTS" ]]; then
        echo "Error: known_hosts file not found: $KNOWN_HOSTS"
        exit 1
    fi
    SSH_MOUNTS="$SSH_MOUNTS -v $(realpath "$KNOWN_HOSTS"):/app/.ssh/known_hosts:ro,Z"
fi

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
echo "  Memory: $MEMORY_LIMIT"
if [[ -n "$SSH_KEY" ]]; then
    echo "  SSH Key: $SSH_KEY -> /app/.ssh/id_rsa"
fi

# Run the container with security options
podman run $DETACH \
    --name "$CONTAINER_NAME" \
    -v "$(realpath "$CONFIG_PATH")":/app/config/config.toml:ro,Z \
    -v "$(realpath "$STATE_PATH")":/app/state:Z \
    $SSH_MOUNTS \
    $WEB_PORT \
    --cap-drop=ALL \
    --security-opt=no-new-privileges:true \
    --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,size=64m \
    --memory="$MEMORY_LIMIT" \
    --memory-swap="$MEMORY_LIMIT" \
    --cpus=1 \
    --pids-limit=100 \
    --ulimit nofile=1024:1024 \
    --ulimit nproc=64:64 \
    --ulimit core=0:0 \
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
