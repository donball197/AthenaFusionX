#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">> [NEXUS] ALPINE PROTOCOL ENGAGED"

# 1. Clean
echo ">> [CLEAN] Scrubbing glibc artifacts..."
rm -rf target/
rm -f *.so

# 2. Build the Alpine SecOps Drone (Once)
# This creates the 50MB replacement for Kali
if ! podman image exists nexus_secops_alpine; then
    echo ">> [HIVE] Forging Alpine SecOps..."
    mkdir -p hives/alpine_secops
    echo "FROM alpine:latest" > hives/alpine_secops/Dockerfile
    echo "RUN apk add --no-cache nmap curl python3 py3-pip bash redis" >> hives/alpine_secops/Dockerfile
    echo "CMD [\"sleep\", \"infinity\"]" >> hives/alpine_secops/Dockerfile
    podman build -t nexus_secops_alpine hives/alpine_secops/
fi

# 3. Build the Core (Static Musl)
echo ">> [BUILD] Compiling Sovereign Binary..."
# Ensure target exists
rustup target add aarch64-unknown-linux-musl
cargo build --target aarch64-unknown-linux-musl --release

# 4. Ignite
echo ">> [IGNITE] Starting Sentry..."
chmod +x ./sentry_safe.sh
./sentry_safe.sh
