#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Removing legacy dead code from neon_math.rs..."

# Remove the unused matvec_bf16_to_f32_neon function block from the end of neon_math.rs
python3 -c '
path = "crates/nexus_edge_minion/src/neon_math.rs"
with open(path, "r") as f:
    lines = f.readlines()

# Filter out lines belonging to matvec_bf16_to_f32_neon
new_lines = []
skip = False
for line in lines:
    if "pub unsafe fn matvec_bf16_to_f32_neon" in line:
        skip = True
    if skip and line.strip() == "}":
        skip = False
        continue
    if not skip:
        new_lines.append(line)

with open(path, "w") as f:
    f.writelines(new_lines)
'

echo ">> [NEXUS] Running Clippy across workspace..."
cargo clippy --workspace --all-targets -j 3 -- -D warnings
