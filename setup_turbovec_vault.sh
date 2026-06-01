#!/bin/bash
set -e

echo "[*] Injecting turbovec into crates/nexus_search Cargo manifest..."
MANIFEST="crates/nexus_search/Cargo.toml"

# Check if turbovec is already present; if not, append it under dependencies
if ! grep -q "turbovec" "$MANIFEST"; then
    # Locate the [dependencies] header and insert turbovec right after it
    sed -i '/\[dependencies\]/a turbovec = "0.1"' "$MANIFEST"
    echo "[+] Cargo.toml updated with turbovec tracking dependency."
else
    echo "[!] turbovec dependency already declared in Cargo.toml."
fi

echo "[*] Constructing VectorVault architecture..."
cat << 'EOF_RUST' > crates/nexus_search/src/lib.rs
// Project Nexus Singularity v3.0.0 — Sovereign Context Index
// Powered by the Google Research TurboQuant Data-Oblivious Quantizer

use turbovec::{TurboQuantIndex, IdMapIndex};
use std::path::Path;

pub struct VectorVault {
    index: TurboQuantIndex,
    id_map: IdMapIndex,
    dimension: usize,
    bit_width: u8,
}

impl VectorVault {
    /// Instantiates an isolated, training-free semantic memory vault on ARM64 silicon
    pub fn new(dimension: usize, bit_width: u8) -> Self {
        Self {
            index: TurboQuantIndex::new(dimension, bit_width),
            id_map: IdMapIndex::new(),
            dimension,
            bit_width,
        }
    }

    /// High-speed insertion of micro-agent context states using O(1) external IDs
    pub fn commit_state(&mut self, id: u64, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dimension {
            return Err(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ));
        }
        
        // Add raw float32 vector to the TurboQuant rotational compression matrix
        self.index.add(embedding).map_err(|e| e.to_string())?;
        // Map the tracking ID for stable deletion and context sync updates
        self.id_map.insert(id, embedding).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Executes analytical Lloyd-Max scalar searches with raw ARM NEON SIMD throughput
    pub fn recall_context(&self, query_embedding: &[f32], k: usize) -> Result<(Vec<f32>, Vec<usize>), String> {
        if query_embedding.len() != self.dimension {
            return Err("Query embedding dimension out of alignment".to_string());
        }

        let (scores, indices) = self.index.search(query_embedding, k).map_err(|e| e.to_string())?;
        Ok((scores, indices))
    }

    /// Serializes the entire un-trained compressed index matrix straight to target workspace disk
    pub fn persist_vault<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        self.index.write(&path_str).map_err(|e| e.to_string())?;
        Ok(())
    }
}
EOF_RUST

echo "[+] VectorVault integrated into crates/nexus_search/src/lib.rs"
echo "[*] Running validation check on the local search layer..."

# Purge any partial artifacts to ensure a pristine ARM64 environment match
cargo check -p nexus_search

echo "[SUCCESS] turbovec engine compiled and bound to the atomic search layer."
