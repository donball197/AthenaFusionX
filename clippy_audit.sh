#!/bin/bash
mkdir -p clippy_logs
CRATES=("identity" "memory_graph" "mesh" "nexus_search" "nexus_substrate" "protocol" "runtime_core")

for crate in "${CRATES[@]}"; do
    echo "Analyzing crate: $crate"
    cargo clippy -p "$crate" --message-format=json > "clippy_logs/${crate}_clippy.json" 2>&1
done

echo "Final integrity verification..."
cargo clippy --workspace -j 1
