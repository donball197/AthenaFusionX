#!/bin/bash
# NEXUS SINGULARITY v1.9 — Universal Ignition Protocol
# Enforces Zero Fragmentation across ARM64 DNA

echo -e "\e[1;36m>> [IGNITION] Initializing ARM64 Substrate Scan...\e[0m"

# Environment Detection
if [ -n "$TERMUX_VERSION" ]; then
    echo -e "\e[32m>> [ENV] Motorola Termux Substrate Detected.\e[0m"
    export NEXUS_ENV="termux"
elif uname -r | grep -q "linux"; then
    echo -e "\e[32m>> [ENV] Chromebook Crostini Substrate Detected.\e[0m"
    export NEXUS_ENV="crostini"
else
    echo -e "\e[33m>> [ENV] Unknown ARM64 Substrate. Defaulting to standard Linux.\e[0m"
    export NEXUS_ENV="generic"
fi

echo -e "\e[33m>> [CLEANSE] Purging Cross-Environment ABI Artifacts...\e[0m"
rm -rf target/
rm -f *.so
rm -f lib/*.so

echo -e "\e[35m>> [MUSCLE] Forging Zig Substrate...\e[0m"
# Compile the Zig muscle into a dynamic library specific to the current hardware
zig build-lib src/kernel/muscle.zig -dynamic -O ReleaseFast
mv libmuscle.so lib/ 2>/dev/null || mv libmuscle.so .

echo -e "\e[34m>> [CORTEX] Igniting Rust Kernel (Gemma 4-31-it active)...\e[0m"
export LD_LIBRARY_PATH="$(pwd)/lib:$(pwd)"
cargo run --release

echo -e "\e[1;32m>> [IGNITION COMPLETE]\e[0m"
