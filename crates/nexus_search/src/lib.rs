// Project Nexus Singularity v3.0.0 — Sovereign Context Index
// Aligned with native turbovec v0.1.3 API definitions

use turbovec::TurboQuantIndex;
use std::collections::HashMap;
use std::path::Path;

pub struct VectorVault {
    index: TurboQuantIndex,
    id_map: HashMap<u64, Vec<f32>>,
    dimension: usize,
    #[allow(dead_code)] bit_width: usize,
}

impl VectorVault {
    /// Instantiates an isolated, training-free semantic memory vault on ARM64 silicon
    pub fn new(dimension: usize, bit_width: usize) -> Self {
        Self {
            index: TurboQuantIndex::new(dimension, bit_width),
            id_map: HashMap::new(),
            dimension,
            bit_width,
        }
    }

    /// High-speed insertion of micro-agent context states using in-place indexing
    pub fn commit_state(&mut self, id: u64, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dimension {
            return Err(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ));
        }
        
        // Add raw float32 vector directly—modifies index state in-place
        self.index.add(embedding);
        
        // Maintain local document matrix mapping for deletion tracking and sync
        self.id_map.insert(id, embedding.to_vec());
        Ok(())
    }

    /// Executes analytical Lloyd-Max scalar searches with raw structural outputs
    pub fn recall_context(&self, query_embedding: &[f32], k: usize) -> Result<(Vec<f32>, Vec<usize>), String> {
        if query_embedding.len() != self.dimension {
            return Err("Query embedding dimension out of alignment".to_string());
        }

        // Extract search structural outputs directly from the SearchResults object
        let results = self.index.search(query_embedding, k);
        
        // Safely cast i64 index elements to usize for our runtime signature compatibility
        let safe_indices: Vec<usize> = results.indices
            .into_iter()
            .map(|idx| idx as usize)
            .collect();
            
        Ok((results.scores, safe_indices))
    }

    /// Serializes the entire un-trained compressed index matrix straight to target workspace disk
    pub fn persist_vault<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        self.index.write(&path_str).map_err(|e| e.to_string())?;
        Ok(())
    }
}
