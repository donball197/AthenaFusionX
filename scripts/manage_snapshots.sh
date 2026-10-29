#!/bin/bash
set -e

CRATES=("crates/runtime_core" "crates/protocol" "crates/nexus_substrate")

# 1. Consolidate/Clean orphaned snapshots
for crate in "${CRATES[@]}"; do
  echo "Cleaning snapshots in $crate..."
  # Logic to identify and remove .snap files not referenced by tests
  find "$crate" -name "*.snap" -type f | while read -r snap; do
    # Simple heuristic: check if corresponding test file exists
    base=$(basename "$snap" .snap)
    if ! grep -rq "$base" "$crate/src"; then
      echo "Removing orphaned snapshot: $snap"
      rm "$snap"
    fi
  done
done

# 2. CI Gate: Run tests and check for drift
echo "Running snapshot tests..."
if ! cargo test --workspace; then
  echo "Error: Snapshot drift detected or tests failed."
  exit 1
fi
