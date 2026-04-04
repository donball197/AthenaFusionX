#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">> [DEV] Compiling Nexus Core..."
cargo build

if [ $? -eq 0 ]; then
    echo ">> [DEV] Ignition Sequence Start..."
    ./target/debug/nexus_singularity
else
    echo ">> [FAIL] Compilation aborted. Check errors."
fi
