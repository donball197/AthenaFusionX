//! Shared marker predicates over the free-form evidence log.
//! Keep local_evidence as String; query it through these helpers only.

/// Successful substrate mutation markers produced by the executor.
pub fn has_mutation_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[WRITE_OK]")
            || l.starts_with("[PATCH_FUNCTION_OK]")
            || l.starts_with("[REPLACE_LINES_OK]")
            || l.starts_with("[DIFF_PATCH_OK]")
    })
}

/// Successful recon / read / search / command markers.
pub fn has_recon_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[READ_OK]")
            || l.starts_with("[READ_CHUNK_OK]")
            || l.starts_with("[SEARCH_CODE_OK]")
            || l.starts_with("[CMD_OK]")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_detects_write_and_patch() {
        assert!(has_mutation_marker("[WRITE_OK] foo.rs\n"));
        assert!(has_mutation_marker(
            "[PATCH_FUNCTION_OK] Updated function 'x' in a.rs\n"
        ));
        assert!(!has_mutation_marker("[READ_OK] foo.rs\n"));
    }

    #[test]
    fn recon_detects_read_search_cmd() {
        assert!(has_recon_marker("[READ_OK] a\n"));
        assert!(has_recon_marker(
            "[SEARCH_CODE_OK] Path: crates | Query: 'x'\n"
        ));
        assert!(has_recon_marker("[CMD_OK] Status: 0\n"));
        assert!(!has_recon_marker("[WRITE_OK] a\n"));
    }
}
