//! Path-grounding check for report artifacts: every workspace path a report
//! cites must exist on disk. Catches hallucinated crate/dir names (e.g. the
//! 2026-11-15 report that cited crates/memory_graph and crates/protocol).

use regex::Regex;
use std::sync::OnceLock;

const ROOTS: &str = "crates|bins|apps|algorithms|drones|scripts|mirror|ui|docs|tests";

/// A line containing one of these is describing absence; its paths are not claims.
const EXEMPT_WORDS: &[&str] = &[
    "does not exist",
    "doesn't exist",
    "nonexistent",
    "non-existent",
    "missing",
    "not found",
    "no such",
    "absent",
    "dangling",
    "orphan",
];

fn token_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(&format!(r#"(?m)(?:^|[\s`'"(\[])((?:{ROOTS})/[A-Za-z0-9_./\-]*[A-Za-z0-9_])"#))
            .expect("path token regex")
    })
}

/// "The `crates/` directory contains ..." style sentences: the backticked names that
/// follow are claimed to be entries of that directory.
fn list_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(crates|bins|apps|algorithms|drones)/`?\s+(?:directory|folder)\s+(?:contains|includes|holds|has)\b",
        )
        .expect("list regex")
    })
}

fn name_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"`([a-z][a-z0-9_-]*)`").expect("name regex"))
}

/// Paths cited in `report` for which `exists` returns false (deduplicated, in order).
pub fn ungrounded_paths(report: &str, exists: impl Fn(&str) -> bool) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for line in report.lines() {
        let lower = line.to_lowercase();
        if EXEMPT_WORDS.iter().any(|w| lower.contains(w)) {
            continue;
        }
        for caps in token_re().captures_iter(line) {
            let m = caps.get(1).expect("group 1");
            if matches!(line[m.end()..].chars().next(), Some('*' | '{' | '<' | '$')) {
                continue; // glob or template, not a concrete path
            }
            let tok = m.as_str();
            if missing.iter().any(|x| x == tok) {
                continue;
            }
            if !exists(tok) {
                missing.push(tok.to_string());
            }
        }
    }
    // Second pass: entries listed under a directory mention.
    for line in report.lines() {
        let lower = line.to_lowercase();
        if EXEMPT_WORDS.iter().any(|w| lower.contains(w)) {
            continue;
        }
        for caps in list_re().captures_iter(line) {
            let whole = caps.get(0).expect("whole match");
            let root = &caps[1];
            let rest = &line[whole.end()..];
            let sentence = rest.split(". ").next().unwrap_or(rest);
            for n in name_re().captures_iter(sentence) {
                let tok = format!("{}/{}", root, &n[1]);
                if !missing.iter().any(|x| x == &tok) && !exists(&tok) {
                    missing.push(tok);
                }
            }
        }
    }
    missing
}

/// Same, resolved against the process working directory (the workspace root).
pub fn ungrounded_paths_on_disk(report: &str) -> Vec<String> {
    ungrounded_paths(report, |p| std::path::Path::new(p).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(p: &str) -> bool {
        ["crates/runtime_core", "crates/runtime_core/src/executor.rs", "bins/ide_backend"]
            .contains(&p)
    }

    #[test]
    fn flags_nonexistent_crates() {
        let r = "The `crates/` directory has `crates/memory_graph` and crates/protocol.";
        assert_eq!(ungrounded_paths(r, fake), vec!["crates/memory_graph", "crates/protocol"]);
    }

    #[test]
    fn existing_paths_pass() {
        let r = "See crates/runtime_core/src/executor.rs and bins/ide_backend.";
        assert!(ungrounded_paths(r, fake).is_empty());
    }

    #[test]
    fn globs_and_templates_are_skipped() {
        assert!(ungrounded_paths("crates/nexus_* are the libraries", fake).is_empty());
        assert!(ungrounded_paths("crates/{a,b}", fake).is_empty());
    }

    #[test]
    fn lines_describing_absence_are_skipped() {
        assert!(ungrounded_paths("crates/old_thing is missing from the workspace", fake).is_empty());
    }

    #[test]
    fn list_items_under_a_directory_are_checked() {
        let r = "The `crates/` directory contains multiple crates including `runtime_core` and `memory_graph`. The `bins/` directory contains `ide_backend`.";
        assert_eq!(ungrounded_paths(r, fake), vec!["crates/memory_graph"]);
    }

    #[test]
    fn duplicates_and_other_roots_are_ignored() {
        let r = "crates/ghost and again crates/ghost; target/release/x; vendor/y";
        assert_eq!(ungrounded_paths(r, fake), vec!["crates/ghost"]);
    }

    #[test]
    fn flags_the_2026_11_15_ungrounded_report() {
        let report = include_str!("../../tests/fixtures/ungrounded_audit_report_2026_11_15.md");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let missing = ungrounded_paths(report, |p| root.join(p).exists());
        assert!(missing.iter().any(|p| p == "crates/memory_graph"), "{missing:?}");
        assert!(missing.iter().any(|p| p == "crates/protocol"), "{missing:?}");
    }

    /// Manual false-positive check:
    ///   GROUNDING_CHECK_FILE=audit_out/prev_audit.md cargo test -p nexus_runtime_core --lib check_file_from_env -- --ignored --nocapture
    #[test]
    #[ignore]
    fn check_file_from_env() {
        let path = std::env::var("GROUNDING_CHECK_FILE").expect("set GROUNDING_CHECK_FILE");
        let report = std::fs::read_to_string(&path).expect("read report");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let missing = ungrounded_paths(&report, |p| root.join(p).exists());
        println!("missing: {missing:?}");
        assert!(missing.is_empty());
    }
}
