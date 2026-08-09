#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Purging Clippy lints..."

# 1. Fix manual_is_multiple_of in safetensors.rs
sed -i 's/self.data.len() % 2 != 0/!self.data.len().is_multiple_of(2)/g' crates/nexus_edge_minion/src/safetensors.rs

# 2. Fix duplicated attributes and empty lines in neon_math.rs
# We will clean up lines 40-52 in neon_math.rs by stripping duplicate #[target_feature] and empty lines.
python3 -c '
path = "crates/nexus_edge_minion/src/neon_math.rs"
with open(path, "r") as f:
    content = f.read()

# Remove the duplicated target_feature and empty lines around matvec_bf16_neon_v2
target_block = """#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[target_feature(enable = "neon")]

unsafe fn matvec_bf16_neon_v2"""

clean_block = """#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn matvec_bf16_neon_v2"""

if target_block in content:
    content = content.replace(target_block, clean_block)
else:
    # Fallback pattern replacement if spacing varies slightly
    content = content.replace("#[target_feature(enable = \"neon\")]\n\n#[target_feature(enable = \"neon\")]", "#[target_feature(enable = \"neon\")]")

with open(path, "w") as f:
    f.write(content)
'

echo ">> [NEXUS] Re-running Clippy..."
cargo clippy --workspace --all-targets -j 3 -- -D warnings
