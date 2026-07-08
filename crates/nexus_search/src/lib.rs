// ⚡ BLAS LINKER FIX: Force LLD to link the math libraries directly
#[link(name = "openblas")]
#[link(name = "cblas")]
extern "C" {}

// Project Nexus Singularity v3.0.0 — Sovereign Context Index
use std::collections::HashMap;
use std::path::Path;
use turbovec::TurboQuantIndex;

pub struct VectorVault {
    index: TurboQuantIndex,
    id_map: HashMap<usize, u64>, // Maps TurboVec Index to SQLite ID!
    current_idx: usize,
    dimension: usize,
    #[allow(dead_code)]
    bit_width: usize,
}

impl VectorVault {
    pub fn new(dimension: usize, bit_width: usize) -> Self {
        Self {
            index: TurboQuantIndex::new(dimension, bit_width),
            id_map: HashMap::new(),
            current_idx: 0,
            dimension,
            bit_width,
        }
    }

    pub fn commit_state(&mut self, id: u64, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dimension {
            return Err(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ));
        }
        self.index.add(embedding);
        self.id_map.insert(self.current_idx, id);
        self.current_idx += 1;
        Ok(())
    }

    pub fn recall_context(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<(Vec<f32>, Vec<u64>), String> {
        if query_embedding.len() != self.dimension {
            return Err("Query embedding mismatch".into());
        }
        let results = self.index.search(query_embedding, k);
        let mut resolved_ids = Vec::new();
        for raw_idx in results.indices {
            if let Some(&real_id) = self.id_map.get(&(raw_idx as usize)) {
                resolved_ids.push(real_id);
            }
        }
        Ok((results.scores, resolved_ids))
    }

    pub fn persist_vault<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        self.index.write(&path_str).map_err(|e| e.to_string())?;
        Ok(())
    }
}
