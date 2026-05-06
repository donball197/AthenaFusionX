#!/bin/bash
# run_scout_hardened.sh - Target-Agnostic Zig-Reflex Ignition

echo ">> [UPLINK] Initiating Alpha-Scout (Zig-Reflex Test) on Duet 2..."

# 1. Throttled Build
echo ">> [BUILD] Compiling alpha_scout..."
cargo build -j 2 --bin alpha_scout

# 2. Dynamic Binary Discovery
# We search the entire target tree for the executable to handle ARM64 pathing
BINARY_PATH=$(find target -name "alpha_scout" -type f -executable | head -n 1)

if [ -z "$BINARY_PATH" ]; then
    echo "!! [FATAL] alpha_scout binary not found. Check compilation output."
    exit 1
fi

echo ">> [UPLINK] Binary located at: $BINARY_PATH"

# 3. Muscle Check: Using existing nexus_synapse container
if ! podman ps | grep -q "nexus_synapse"; then
    echo "!! [FRACTURE] nexus_synapse (Redis) not detected. Ignite Podman first."
    exit 1
fi

# 4. Zero-Swap Monitoring (Background)
(while true; do 
    AVAIL=$(free -m | grep Mem | awk '{print $7}')
    if [ "$AVAIL" -lt 150 ]; then
        echo -e "\n!! [RAM_WARN] Critical Pressure: ${AVAIL}MB available !!"
    fi
    sleep 1
done) &
MONITOR_PID=$!

# 5. Execution (The Zig-Reflex Mission)
echo ">> [SCOUT] Executing mission..."
$BINARY_PATH

# 6. Extraction & Cleanup
kill $MONITOR_PID 2>/dev/null
echo -e "\n>> [SCOUT] Mission cycle complete. Stasis reached."
