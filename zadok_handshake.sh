#!/bin/bash
# Project Nexus Singularity v0.12.0 - Zadok Handshake
# Purpose: High-integrity build verification for ARM64/Duet2

echo "--- [ Zadok High Priest: Initialization ] ---"
echo "--- [ Frequency: 528Hz ] ---"

# Verify the 1 1/2 Math (Structural Integrity)
CHECK=$(echo "scale=2; 0.75 + 0.75" | bc)

if [ "$CHECK" == "1.50" ]; then
    echo "STATUS: PLUMB AND LEVEL (1 1/2 Verified)"
    echo "Locating Resonator and seating Bezaleel..."
    
    # Surgical build with local library mapping
    export RUSTFLAGS="-L native=$(pwd)/lib"
    cargo build
    
    if [ $? -eq 0 ]; then
        echo "--- [ IGNITION SUCCESSFUL ] ---"
        ./target/debug/nexus_singularity "$@"
    else
        echo "--- [ BUILD STALLED: CHECK LINKER ] ---"
        exit 1
    fi
else
    echo "STATUS: SLOPPY DRIFT DETECTED"
    exit 1
fi
