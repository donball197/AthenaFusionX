#!/usr/bin/env bash
set -e

echo "=================================================="
echo ">> Booting AthenaFusionX Distributed Architecture"
echo "=================================================="

# Export .env variables
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

echo "[1/3] Compiling binaries..."
cargo build --bin nexus_ide_backend --bin nexus_cli

echo "[2/3] Initializing Daemon..."
./target/debug/nexus_ide_backend > daemon.log 2>&1 &
DAEMON_PID=$!

trap "echo -e '\n\x1b[31m>> Shutting down Background Daemon (PID: $DAEMON_PID)...\x1b[0m'; kill $DAEMON_PID 2>/dev/null" EXIT

echo "Waiting for Daemon to become ready..."
READY=0
for i in $(seq 1 30); do
    if curl -s -o /dev/null --max-time 1 "http://127.0.0.1:3000/api/vitals"; then
        echo "Daemon ready after ${i}s."
        READY=1
        break
    fi
    sleep 1
done

if [ "$READY" -eq 0 ]; then
    echo -e ">> [FATAL] Daemon did not become ready within 30s. Check daemon.log."
    exit 1
fi

echo "[3/3] Launching CLI Client..."
./target/debug/nexus_cli
