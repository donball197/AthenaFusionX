#!/usr/bin/env bash
set -e

echo ">> 1. Testing Raw Host Curl..."
curl -I -s https://example.com | head -n 1

echo ">> 2. Testing Podman Alpine Wget..."
podman run --rm alpine:latest wget -qO- https://example.com | head -n 5

echo ">> 3. Testing Podman Alpine Curl Installation & Fetch..."
podman run --rm alpine:latest sh -c "apk add --no-cache curl && curl -s https://example.com | head -n 5"

echo ">> SANDBOX NETWORK INTEGRITY TEST COMPLETE."
