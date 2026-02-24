#!/bin/bash
# NEXUS SENTRY // ALPINE MUSL EDITION

echo ">> [SENTRY] Watchdog Online."

# Determine Binary Path (Musl vs Glibc)
if [ -f "./target/aarch64-unknown-linux-musl/release/nexus_singularity" ]; then
    BIN="./target/aarch64-unknown-linux-musl/release/nexus_singularity"
else
    BIN="./target/release/nexus_singularity"
fi

CRASH_COUNT=0
MAX_CRASHES=3

while true; do
    echo ">> [SENTRY] Igniting Core: $BIN"
    
    if [ ! -f "$BIN" ]; then
        echo ">> [SENTRY] Binary missing. Run ./nexus.sh first."
        exit 1
    fi

    START_TIME=$(date +%s)
    $BIN
    EXIT_CODE=$?
    END_TIME=$(date +%s)
    
    RUNTIME=$((END_TIME - START_TIME))
    echo ">> [SENTRY] Core offline (Run time: ${RUNTIME}s)"

    if [ $RUNTIME -lt 10 ]; then
        ((CRASH_COUNT++))
        if [ $CRASH_COUNT -ge $MAX_CRASHES ]; then
            echo "!! [CRITICAL] SYSTEM UNSTABLE."
            exit 1
        fi
        sleep 5
    else
        CRASH_COUNT=0
        sleep 2
    fi
done
