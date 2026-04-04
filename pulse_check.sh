#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [INITIATING KERNEL]"
# Launch kernel in background
./target/aarch64-unknown-linux-gnu/release/nexus_singularity & 
KERNEL_PID=$!

# Wait for port 3000 to open
sleep 3

echo ">>> [VERIFYING ASSET HEADERS]"
JS_FILE=$(grep -oP '_app/immutable/entry/start\.\w+\.js' ui/dist/index.html | head -n 1)
curl -I "http://localhost:3000/$JS_FILE"

echo ">>> [TERMINATING TEST]"
kill $KERNEL_PID
