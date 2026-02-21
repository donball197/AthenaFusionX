#!/bin/bash
# NEXUS SENTRY // RELIABLE WATCHDOG

echo ">> [SENTRY] Watchdog Online. Manual Mode Engaged."

while true; do
    echo ">> [SENTRY] Igniting Nexus Core..."
    
    # Run the server. No auto-patching allowed.
    cargo run --release --bin nexus_singularity
    
    echo ">> [SENTRY] Crash detected. Holding for 3 seconds before reboot..."
    sleep 3
done
