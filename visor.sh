#!/bin/bash
echo ">> [SYSTEM] INITIATING SOVEREIGN REBUILD..."
cd ui && npm run build && cd ..
if [ $? -eq 0 ]; then
    echo ">> [SYSTEM] UI LAYER COMPILED. LAUNCHING NODE..."
    cargo run
else
    echo ">> [ERROR] UI BUILD FAILED. ABORTING LAUNCH."
fi
