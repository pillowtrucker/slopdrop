#!/bin/bash
# Build the Slopdrop container image
# Usage: ./container-build.sh [tag]

set -e

IMAGE_NAME="slopdrop"
TAG="${1:-latest}"

echo "Building Slopdrop container image: ${IMAGE_NAME}:${TAG}"

podman build \
    --tag "${IMAGE_NAME}:${TAG}" \
    --file Containerfile \
    --format docker \
    .

echo ""
echo "Build complete!"
echo "Image: ${IMAGE_NAME}:${TAG}"
echo ""
echo "To run the container, use:"
echo "  ./container-run.sh"
echo ""
echo "Or manually:"
echo "  podman run -d \\"
echo "    -v ./config.toml:/app/config/config.toml:ro \\"
echo "    -v ./state:/app/state:Z \\"
echo "    ${IMAGE_NAME}:${TAG}"
