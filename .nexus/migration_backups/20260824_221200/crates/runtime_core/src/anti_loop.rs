//! Strong anti-loop + cheap search-result relevance scoring.
//!
//! Prevents the strategist from repeating the identical tool call forever
//! and stops LICENSE / Cargo.lock matches from raising belief.

use std::collections::HashMap;

/// Tracks recent tool invocations. Call `record` after every tool result.
#[derive(Debug, Default)]
pub struct AntiLoopGuard {
    /// key → consecutive count
    counts: HashMap<String, u32>,
    /// Maximum identical calls before forced pivot
    pub max_identical: u32,
}

impl AntiLoopGuard {
    pub fn new(max_identical: u32) -> Self {
        Self {
            counts: HashMap::new(),
            max_identical: max_identical.max(2),
        }
    }

    /// Returns true if this call is ALLOWED. Returns false if the guard
    /// demands a pivot (caller must choose a different tool/args).
    pub fn record(&mut self, tool: &str, path: &str, query: &str) -> bool {
        let key = format!("{tool}|{path}|{query}").to_ascii_lowercase();
        let c = self.counts.entry(key).or_insert(0);
        *c += 1;
        // Decay others slightly so the map doesn't grow forever
        if self.counts.len() > 64 {
            self.counts.retain(|_, v| {
                *v = v.saturating_sub(1);
                *v > 0
            });
        }
        *c <= self.max_identical
    }

    pub fn reset(&mut self) {
        self.counts.clear();
    }
}

/// Cheap heuristic relevance score for search_code / grep output.
/// Returns 0.0 ..= 1.0
///
/// LICENSE, Cargo.lock, binary files, and pure copyright text score near 0.
/// Hits that look like definitions (struct/enum/fn/impl/mod) score high.
pub fn score_search_relevance(query: &str, raw_output: &str) -> f32 {
    if raw_output.trim().is_empty() {
        return 0.0;
    }
    let lower = raw_output.to_ascii_lowercase();
    let q = query.to_ascii_lowercase();

    // Hard penalties
    let mut score: f32 = 0.35;
    if lower.contains("license") || lower.contains("copyright (c)") || lower.contains("all rights reserved") {
        score -= 0.25;
    }
    if lower.contains("cargo.lock") || lower.contains("[[package]]") {
        score -= 0.20;
    }
    if lower.contains(".png") || lower.contains(".jpg") || lower.contains("binary file") {
        score -= 0.30;
    }

    // Definition-like hits
    let def_markers = ["struct ", "enum ", "fn ", "impl ", "mod ", "type ", "trait ", "pub struct", "pub enum", "pub fn"];
    let mut def_hits = 0;
    for m in def_markers {
        if lower.contains(m) {
            def_hits += 1;
        }
    }
    score += (def_hits as f32) * 0.08;

    // Query token present in a code-ish line
    if !q.is_empty() && lower.contains(&q) {
        score += 0.15;
    }

    score.clamp(0.0, 1.0)
}

/// Only allow belief increases when relevance >= threshold.
pub fn should_raise_belief(relevance: f32, threshold: f32) -> bool {
    relevance >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anti_loop_trips() {
        let mut g = AntiLoopGuard::new(3);
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(!g.record("search_code", "apps/warp", "state"));
    }

    #[test]
    fn license_is_low_relevance() {
        let out = "LICENSE-AGPL\nCopyright (C) 2024\nAll rights reserved\n";
        let s = score_search_relevance("state", out);
        assert!(s < 0.3, "LICENSE should be low relevance, got {s}");
    }

    #[test]
    fn struct_def_is_high_relevance() {
        let out = "src/foo.rs:12: pub struct CorticalLoop {\nsrc/foo.rs:13:     client: Client,\n";
        let s = score_search_relevance("CorticalLoop", out);
        assert!(s >= 0.5, "definition should be high relevance, got {s}");
    }
}
