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

# Poll the daemon until it actually answers on port 3000, instead of guessing a fixed delay
echo "Waiting for Daemon to become ready..."
READY=0
for i in $(seq 1 30); do
    if curl -s -o /dev/null --max-time 1 "http://localhost:3000/vitals"; then
        echo "Daemon ready after ${i}s."
        READY=1
        break
    fi
    sleep 1
done
if [ "$READY" -eq 0 ]; then
    echo -e "[31m>> [FATAL] Daemon did not become ready within 30s. Check daemon.log.[0m"
    exit 1
fi

echo "[3/3] Launching CLI Client..."
./target/debug/nexus_cli
