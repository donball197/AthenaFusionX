//! Deterministic reconnaissance-planning algorithms.
//!
//! This crate converts physical search evidence into candidate
//! read strategies for the cognitive coordinator.
//!
//! It deliberately has no dependency on runtime_core.
//! It does not execute reads, inspect the filesystem, mutate the
//! hypothesis graph, assign belief, authorize actions, or decide
//! completion.

/// Deterministic planner for post-search reconnaissance.
///
/// The planner answers one narrow cognitive question:
///
/// "A code search produced physical MATCHES evidence. Which discovered
/// source paths have not yet been read, and therefore represent useful
/// next reconnaissance strategies?"
pub struct ReconnaissancePlanner;

impl ReconnaissancePlanner {
    /// Convert search evidence into ordered `read_file:<path>` strategies.
    ///
    /// The parser preserves the existing Cortex behavior:
    /// - only `[SEARCH_CODE_OK]` MATCHES blocks are considered;
    /// - both `path:line:text` and `path-line-text` forms are accepted;
    /// - source candidates are restricted to `.rs`, `.toml`, `.md`, `.txt`;
    /// - duplicate paths are removed;
    /// - at most eight discovered paths are retained;
    /// - already-read paths are excluded;
    /// - at most `max_candidates` read strategies are returned.
    pub fn plan(evidence: &str, max_candidates: usize) -> Vec<String> {
        if max_candidates == 0 {
            return Vec::new();
        }

        let paths = Self::extract_search_hit_paths(evidence);

        paths
            .into_iter()
            .filter(|path| !Self::path_already_read(evidence, path))
            .take(max_candidates)
            .map(|path| format!("read_file:{}", path))
            .collect()
    }

    fn extract_search_hit_paths(evidence: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut in_matches = false;

        for line in evidence.lines() {
            if line.starts_with("[SEARCH_CODE_OK]") {
                in_matches = false;
                continue;
            }

            if line.trim() == "MATCHES:" {
                in_matches = true;
                continue;
            }

            if !in_matches || line.trim().is_empty() {
                continue;
            }

            if line.starts_with('[') {
                in_matches = false;
                continue;
            }

            // ripgrep-style: path:line:text
            // or path-line-text
            let candidate = line
                .split_once(':')
                .map(|(p, _)| p)
                .or_else(|| line.split_once('-').map(|(p, _)| p))
                .unwrap_or(line)
                .trim();

            let looks_like_source = candidate.contains('/')
                && (candidate.ends_with(".rs")
                    || candidate.ends_with(".toml")
                    || candidate.ends_with(".md")
                    || candidate.ends_with(".txt"));

            if looks_like_source && !out.iter().any(|e| e == candidate) {
                out.push(candidate.to_string());
            }

            if out.len() >= 8 {
                break;
            }
        }

        out
    }

    fn path_already_read(evidence: &str, path: &str) -> bool {
        evidence.lines().any(|line| {
            (line.starts_with("[READ_OK]") && line.contains(path))
                || (line.starts_with("[READ_CHUNK_OK] File: ") && line.contains(path))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ReconnaissancePlanner;

    #[test]
    fn plans_unread_search_hits_in_order() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: Cargo.toml
MATCHES:
vendor/openfang/Cargo.toml:1:[package]
vendor/openfang/src/main.rs:10:foo
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(
            actual,
            vec!["read_file:vendor/openfang/Cargo.toml", "read_file:vendor/openfang/src/main.rs",]
        );
    }

    #[test]
    fn excludes_already_read_paths() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: Cargo.toml
MATCHES:
vendor/openfang/Cargo.toml:1:[package]
vendor/openfang/src/main.rs:10:foo
[READ_OK] vendor/openfang/Cargo.toml
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(actual, vec!["read_file:vendor/openfang/src/main.rs"]);
    }

    #[test]
    fn recognizes_read_chunk_evidence() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: Cargo.toml
MATCHES:
vendor/openfang/Cargo.toml:1:[package]
vendor/openfang/src/main.rs:10:foo
[READ_CHUNK_OK] File: vendor/openfang/Cargo.toml
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(actual, vec!["read_file:vendor/openfang/src/main.rs"]);
    }

    #[test]
    fn deduplicates_search_hits() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: Cargo.toml
MATCHES:
vendor/openfang/Cargo.toml:1:[package]
vendor/openfang/Cargo.toml:8:[workspace]
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(actual, vec!["read_file:vendor/openfang/Cargo.toml"]);
    }

    #[test]
    fn ignores_non_source_candidates() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: Cargo.toml
MATCHES:
vendor/openfang/image.png:1:binary
vendor/openfang/Cargo.toml:1:[package]
vendor/openfang/src/main.rs:10:foo
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(
            actual,
            vec!["read_file:vendor/openfang/Cargo.toml", "read_file:vendor/openfang/src/main.rs",]
        );
    }

    #[test]
    fn respects_candidate_limit() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: source
MATCHES:
vendor/openfang/a.rs:1:a
vendor/openfang/b.rs:1:b
vendor/openfang/c.rs:1:c
";

        let actual = ReconnaissancePlanner::plan(evidence, 2);

        assert_eq!(
            actual,
            vec!["read_file:vendor/openfang/a.rs", "read_file:vendor/openfang/b.rs",]
        );
    }

    #[test]
    fn stops_at_next_evidence_marker() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: source
MATCHES:
vendor/openfang/a.rs:1:a
[READ_OK] vendor/openfang/other.rs
vendor/openfang/b.rs:1:b
";

        let actual = ReconnaissancePlanner::plan(evidence, 4);

        assert_eq!(actual, vec!["read_file:vendor/openfang/a.rs"]);
    }

    #[test]
    fn zero_limit_returns_no_strategies() {
        let evidence = "\
[SEARCH_CODE_OK] Path: vendor/openfang | Query: source
MATCHES:
vendor/openfang/a.rs:1:a
";

        assert!(ReconnaissancePlanner::plan(evidence, 0).is_empty());
    }
}
