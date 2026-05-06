#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1

# ====================== GHOST LAYER (Highest Priority) ======================
if [ "$1" = "ghost" ]; then
    shift
    echo ">> [NEXUS] SPECTRAL HANDOFF — GHOST v4.2.1 + ZIRCON VMO"
    if [ -x "./nexus_ghost_core.sh" ]; then
        ./nexus_ghost_core.sh "$@"
    else
        echo ">> [ERROR] nexus_ghost_core.sh not found"
    fi
    exit 0
fi

# ====================== ORIGINAL NEXUS BUILD FLOW ======================
echo ">> [NEXUS] ALPINE PROTOCOL ENGAGED"

# 1. Clean
echo ">> [CLEAN] Scrubbing glibc artifacts..."
rm -rf target/
rm -f *.so

# 2. Alpine SecOps Drone
if ! podman image exists nexus_secops_alpine; then
    echo ">> [HIVE] Forging Alpine SecOps..."
    mkdir -p hives/alpine_secops
    echo "FROM alpine:latest" > hives/alpine_secops/Dockerfile
    echo "RUN apk add --no-cache nmap curl python3 py3-pip bash redis" >> hives/alpine_secops/Dockerfile
    echo "CMD [\"sleep\", \"infinity\"]" >> hives/alpine_secops/Dockerfile
    podman build -t nexus_secops_alpine hives/alpine_secops/
fi

# 3. Build Core
echo ">> [BUILD] Compiling Sovereign Binary..."
rustup target add aarch64-unknown-linux-musl
cargo build --target aarch64-unknown-linux-musl --release

# 4. Ignite Sentry
echo ">> [IGNITE] Starting Sentry Watchdog..."
chmod +x ./sentry_safe.sh 2>/dev/null || true
./sentry_safe.sh
