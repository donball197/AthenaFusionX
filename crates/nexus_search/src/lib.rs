#[cfg(test)]
extern crate openblas_src;

// ⚡ BLAS LINKER FIX: Force LLD to link the math libraries directly
#[link(name = "openblas")]
#[link(name = "cblas")]
extern "C" {}

use std::collections::HashMap;
use std::path::Path;
use turbovec::TurboQuantIndex;

pub struct VectorVault {
    index: TurboQuantIndex,
    id_map: HashMap<usize, u64>, // Maps TurboVec index slot to SQLite ID
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

    pub fn commit_state(&mut self, sqlite_id: u64, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dimension {
            return Err(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ));
        }
        self.index.add(embedding);
        self.id_map.insert(self.current_idx, sqlite_id);
        self.current_idx += 1;
        Ok(())
    }

    /// Add many vectors in ONE index.add call (amortizes per-call setup).
    pub fn commit_batch(&mut self, items: &[(u64, Vec<f32>)]) -> Result<(), String> {
        let mut flat: Vec<f32> = Vec::with_capacity(items.len() * self.dimension);
        for (_, v) in items {
            if v.len() != self.dimension {
                return Err("Dimension mismatch in batch".into());
            }
            flat.extend_from_slice(v);
        }
        self.index.add(&flat);
        for (id, _) in items {
            self.id_map.insert(self.current_idx, *id);
            self.current_idx += 1;
        }
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

        // Keep scores and SQLite IDs positionally aligned. A missing ID-map
        // entry must not leave an orphaned score behind.
        let mut scores = Vec::new();
        let mut resolved_ids = Vec::new();

        for (raw_idx, score) in results.indices.into_iter().zip(results.scores) {
            if let Some(&real_id) = self.id_map.get(&(raw_idx as usize)) {
                scores.push(score);
                resolved_ids.push(real_id);
            }
        }

        Ok((scores, resolved_ids))
    }

    pub fn persist_vault<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        self.index.write(&path_str).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::VectorVault;

    fn vector(value: f32) -> Vec<f32> {
        let mut v = vec![0.0; 384];
        v[0] = value;
        v
    }

    #[test]
    fn rejects_wrong_embedding_dimension_on_commit() {
        let mut vault = VectorVault::new(384, 4);
        let result = vault.commit_state(42, &[0.0; 383]);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Dimension mismatch: expected 384, got 383");
    }

    #[test]
    fn rejects_wrong_query_dimension() {
        let vault = VectorVault::new(384, 4);
        let result = vault.recall_context(&[0.0; 383], 1);

        assert_eq!(result.unwrap_err(), "Query embedding mismatch");
    }

    #[test]
    fn resolves_committed_sqlite_id() {
        let mut vault = VectorVault::new(384, 4);
        vault.commit_state(1234, &vector(1.0)).unwrap();

        let (scores, ids) = vault.recall_context(&vector(1.0), 1).unwrap();

        assert_eq!(scores.len(), ids.len());
        assert_eq!(ids, vec![1234]);
    }

    #[test]
    fn missing_id_mapping_does_not_leave_orphaned_score() {
        let mut vault = VectorVault::new(384, 4);
        vault.commit_state(1234, &vector(1.0)).unwrap();

        // Simulate an inconsistent slot-to-SQLite mapping. The recall result
        // must drop the entire unmatched result, not just its ID.
        vault.id_map.remove(&0);

        let (scores, ids) = vault.recall_context(&vector(1.0), 1).unwrap();

        assert!(scores.is_empty());
        assert!(ids.is_empty());
        assert_eq!(scores.len(), ids.len());
    }
}
