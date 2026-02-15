#!/bin/bash

# ==============================================================================
# NEXUS MAINTENANCE PROTOCOL
# TARGET: Lenovo Duet 2 (Low RAM Optimization)
# ==============================================================================

echo ">> [NEXUS] INITIATING MAINTENANCE SEQUENCE..."

# 1. PULL LATEST CODE
echo ">> [GIT] Pulling latest neural pathways..."
if command -v git &> /dev/null; then
    git pull
else
    echo ">> [WARN] Git not found. Skipping download."
fi

# 2. CHECK MEMORY SWAP (Critical for Compilation)
SWAP_SIZE=$(free -m | grep Swap | awk '{print $2}')
if [ "$SWAP_SIZE" -eq "0" ]; then
    echo ">> [DANGER] No Swap detected! Compilation may crash."
    echo "   Please run: sudo fallocate -l 2G /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile"
    read -p "   Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 3. REBUILD FRONTEND (UI)
echo ">> [UI] Recompiling Interface..."
cd ui
if [ -f "package.json" ]; then
    npm install
    npm run build
    if [ $? -eq 0 ]; then
        echo ">> [UI] Build Successful."
    else
        echo ">> [UI] Build Failed. Check logs."
        exit 1
    fi
else
    echo ">> [UI] No UI source detected. Skipping."
fi
cd ..

# 4. REBUILD BACKEND (RUST)
echo ">> [CORE] Recompiling Singularity Engine..."
# Force single-core build to prevent OOM (Out of Memory) kills on 4GB RAM
export CARGO_BUILD_JOBS=1
export RUSTFLAGS="-C codegen-units=1 -C debuginfo=0"

cargo build --release

if [ $? -eq 0 ]; then
    echo ">> [SUCCESS] System Upgrade Complete."
    echo ">> [INFO] Run ./status.sh to verify."
else
    echo ">> [FAIL] Core compilation failed."
fi
