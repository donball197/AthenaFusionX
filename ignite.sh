#!/bin/bash
echo -e "\033[1;35m>> [NEXUS] INITIATING TOTAL IGNITION SEQUENCE...\033[0m"

# 1. Clear old tokens and locks
rm -f .nexus_ui_token
rm -f .nexus_memory.db-shm .nexus_memory.db-wal

# 2. Build the UI
echo ">> [UI] Rebuilding Svelte Substrate..."
cd ui && npm install && npm run build && cd ..

# 3. Prime the WASM cells
echo ">> [FORGE] Priming WASM standard library..."
cargo run --bin prime_wasm

# 4. Launch the Kernel
echo ">> [UPLINK] Starting main server..."
cargo run
