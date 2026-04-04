#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">> [SYSTEM] INITIATING SOVEREIGN REBUILD..."
cd ui && npm run build && cd ..
if [ $? -eq 0 ]; then
    echo ">> [SYSTEM] UI LAYER COMPILED. LAUNCHING NODE..."
    cargo run
else
    echo ">> [ERROR] UI BUILD FAILED. ABORTING LAUNCH."
fi
