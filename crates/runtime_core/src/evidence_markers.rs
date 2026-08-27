#![allow(clippy::items_after_test_module)]
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
        assert!(has_mutation_marker("[PATCH_FUNCTION_OK] Updated function 'x' in a.rs\n"));
        assert!(!has_mutation_marker("[READ_OK] foo.rs\n"));
    }

    #[test]
    fn recon_detects_read_search_cmd() {
        assert!(has_recon_marker("[READ_OK] a\n"));
        assert!(has_recon_marker("[SEARCH_CODE_OK] Path: crates | Query: 'x'\n"));
        assert!(has_recon_marker("[CMD_OK] Status: 0\n"));
        assert!(!has_recon_marker("[WRITE_OK] a\n"));
    }
}

/// PATCH 1a: Returns true if `evidence` contains a successful write marker
/// for a path matching `target_path_substr`. Matches by full path, basename,
/// or suffix.
pub fn has_artifact_write(evidence: &str, target_path_substr: &str) -> bool {
    let target = target_path_substr.trim().trim_start_matches("./");
    if target.is_empty() {
        return false;
    }
    let target_lower = target.to_lowercase();
    let markers = [
        "[write_ok]",
        "[diff_patch_ok]",
        "[patch_function_ok]",
        "[replace_lines_ok]",
        "[write_noop]",
        "synthesized and wrote report to",
    ];
    for line in evidence.lines() {
        let line_lower = line.to_lowercase();
        if !markers.iter().any(|m| line_lower.contains(m)) {
            continue;
        }
        if line_lower.contains(&target_lower) {
            return true;
        }
        if let Some(basename) = std::path::Path::new(target).file_name().and_then(|f| f.to_str()) {
            if line_lower.contains(&basename.to_lowercase()) {
                return true;
            }
        }
    }
    false
}
