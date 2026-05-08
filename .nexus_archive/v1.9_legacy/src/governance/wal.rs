// src/governance/wal.rs — v1.2 [CORRECT_IMPORTS]
//
// FIX over v1 script: `use std::io::Write` must be at module level, not inside
// the impl block. The original patch had it inside append() which is invalid Rust.
use std::fs::OpenOptions;
use std::io::Write;        // ← hoisted to module level

pub struct MerkleWAL;

impl MerkleWAL {
    pub fn new() -> Self { Self }

    /// append: Writes one JSON-line per governance event to .nexus/governance.wal.
    /// Non-fatal on I/O error — governance logging must never crash the runtime.
    pub fn append(&mut self, patch: String) {
        std::fs::create_dir_all(".nexus").ok();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".nexus/governance.wal")
        {
            let _ = writeln!(file, "{}", patch);
        }
    }

    /// read_all: Returns all WAL lines for offline audit or replay.
    pub fn read_all() -> Vec<String> {
        std::fs::read_to_string(".nexus/governance.wal")
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect()
    }
}
