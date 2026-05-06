#!/usr/bin/env bash
set -e

# Environment Detection & Cache Routing (Zero Fragmentation)
if [[ "$PREFIX" == *com.termux* ]]; then
    export NEXUS_ENV="Motorola_Termux"
    export CARGO_TARGET_DIR="/tmp/nexus-target-motorola"
else
    export NEXUS_ENV="Chromebook_Crostini"
    export CARGO_TARGET_DIR="/tmp/nexus-target-crostini"
fi

mkdir -p "$CARGO_TARGET_DIR"
echo -e "\x1b[36m>> [IGNITION] Environment Detected: $NEXUS_ENV\x1b[0m"
echo -e "\x1b[36m>> [IGNITION] Compilation Cache Routed: $CARGO_TARGET_DIR\x1b[0m"

COMMAND=${1:-run}

if [ "$COMMAND" == "build" ]; then
    echo -e "\x1b[35m>> [BUILD] Forging Workspace Binaries...\x1b[0m"
    if [ -d "bins/node" ]; then
        cargo build -p nexus_node -p nexus_cli -p nexus_ide_backend --release -j 1
    else
        cargo build --bin nexus -j 1
    fi
    echo -e "\x1b[32m>> [SUCCESS] Build Complete.\x1b[0m"

elif [ "$COMMAND" == "run" ] || [ "$COMMAND" == "dev" ]; then
    echo -e "\x1b[35m>> [RUN] Booting Nexus Edge Node...\x1b[0m"
    if [ -d "bins/node" ]; then
        cargo run -p nexus_node -j 1
    else
        cargo run --bin nexus -j 1
    fi

elif [ "$COMMAND" == "container" ]; then
    echo -e "\x1b[35m>> [PODMAN] Forging Sovereign Container Image...\x1b[0m"
    # Execute rootless Podman build using the local Containerfile
    podman build -f Containerfile -t nexus_node:v3.0.0 .
    echo -e "\x1b[32m>> [SUCCESS] Podman image enshrined.\x1b[0m"

elif [ "$COMMAND" == "clean" ]; then
    echo -e "\x1b[31m>> [PURGE] Eradicating Local Caches...\x1b[0m"
    rm -rf "$CARGO_TARGET_DIR"
    rm -f *.so
    echo -e "\x1b[32m>> [SUCCESS] Zero Fragmentation enforced. Caches cleared.\x1b[0m"
else
    echo -e "\x1b[33mUsage: nexus-dev [build|run|container|clean]\x1b[0m"
fi
