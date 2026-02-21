#!/bin/bash
# NEXUS SENTRY // SAFETY PROTOCOL ENGAGED

echo ">> [SENTRY] Watchdog Online. Monitoring Nexus Core..."

CRASH_COUNT=0
MAX_CRASHES=3

while true; do
    echo ">> [SENTRY] Igniting Nexus Core..."
    
    # Capture start time
    START_TIME=$(date +%s)
    
    # Run the binary directly (faster restart than recompiling)
    # Ensure you ran 'cargo build --release' first!
    if [ -f "./target/release/nexus_singularity" ]; then
        ./target/release/nexus_singularity
    else
        echo ">> [SENTRY] Binary not found. Compiling first..."
        cargo run --release --bin nexus_singularity
    fi
    
    EXIT_CODE=$?
    END_TIME=$(date +%s)
    RUNTIME=$((END_TIME - START_TIME))

    echo ">> [SENTRY] Core offline. Exit Code: $EXIT_CODE. Runtime: ${RUNTIME}s"

    # Safety Circuit Breaker
    if [ $RUNTIME -lt 10 ]; then
        ((CRASH_COUNT++))
        echo "!! [WARNING] Rapid Disassembly Detected ($CRASH_COUNT/$MAX_CRASHES)"
        if [ $CRASH_COUNT -ge $MAX_CRASHES ]; then
            echo "!! [CRITICAL] SYSTEM UNSTABLE. ABORTING AUTO-RESTART."
            exit 1
        fi
        echo ">> [SENTRY] Cooling down for 10 seconds..."
        sleep 10
    else
        # Reset counter if it ran successfully for a while
        CRASH_COUNT=0
        echo ">> [SENTRY] Rebooting in 3 seconds..."
        sleep 3
    fi
done
