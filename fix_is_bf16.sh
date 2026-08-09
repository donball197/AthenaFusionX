#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Removing unused is_bf16_supported() helper..."

python3 -c '
path = "crates/nexus_edge_minion/src/neon_math.rs"
with open(path, "r") as f:
    content = f.read()

target = """#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn is_bf16_supported() -> bool {
    std::arch::is_aarch64_feature_detected!("bf16")
}"""

if target in content:
    content = content.replace(target, "")

with open(path, "w") as f:
    f.write(content)
'

echo ">> [NEXUS] Re-running workspace Clippy..."
cargo clippy --workspace --all-targets -j 3 -- -D warnings
