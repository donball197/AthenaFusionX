#!/usr/bin/env bash

echo "=================================================="
echo ">> Booting AthenaFusionX Distributed Architecture"
echo "=================================================="

echo "[1/3] Compiling binaries..."
cargo build --bin nexus_ide_backend --bin nexus_cli

echo "[2/3] Initializing Daemon..."
# Start the daemon in the background and pipe its internal logs to a file
./target/debug/nexus_ide_backend > daemon.log 2>&1 &
DAEMON_PID=$!

# Trap ensures that when the CLI exits, the Daemon is killed automatically
trap "echo -e '\n\x1b[31m>> Shutting down Background Daemon (PID: $DAEMON_PID)...\x1b[0m'; kill $DAEMON_PID 2>/dev/null" EXIT

# Give the daemon 3 seconds to boot the database and open port 3000
sleep 3

echo "[3/3] Launching CLI Client..."
./target/debug/nexus_cli
