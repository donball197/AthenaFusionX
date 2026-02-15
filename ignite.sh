#!/bin/bash
echo -e "\033[1;36m>> [NEXUS] SYNCHRONIZING SHARED SOURCE...\033[0m"

# 1. Clear Port 3000
fuser -k 3000/tcp 2>/dev/null

# 2. Build the Visor (Sovereign Build)
# We go INTO the ui directory to use its node_modules and config
cd ui
echo ">> [VISOR] COMPILING FROM ui/src..."
npm run build -- --outDir dist --emptyOutDir
cd ..

# 3. Launch the Brain
# This serves the 'ui/dist' folder we just built
echo ">> [KERNEL] IGNITING RUST ENGINE..."
export CARGO_BUILD_JOBS=1
cargo run --release
