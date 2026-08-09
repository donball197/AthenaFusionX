#!/bin/bash
set -euo pipefail

echo ">> [FIX] Correcting should_panic attribute syntax in rope_golden_tests.rs..."
sed -i 's/expected: /expected = /g' crates/nexus_edge_minion/tests/rope_golden_tests.rs

echo ">> [PHASE 2] Re-running golden-vector test suite (Hard Gate)..."
cargo test -p nexus_edge_minion --test rope_golden_tests
