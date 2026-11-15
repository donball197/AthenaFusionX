//! Forbidden-removal scan for halt reasoning.
//! Port of /tmp/patch_c_semantics_v2.py (21/21 in Python). Rust version NOT yet compiled.
//!
//! Drop into crates/runtime_core/src/cortical/ (or paste into halt_claims.rs).
//! Requires the `regex` crate. Rust >= 1.70 (OnceLock).
//!
//! Tripwire, not proof: lexical matching only. See "Known gaps" in the PR notes.

use regex::Regex;
use std::sync::OnceLock;

const VERBS: &str = r"(?:remov(?:e|es|ing)|delet(?:e|es|ing)|wip(?:e|es|ing)|purg(?:e|es|ing)|drop(?:s|ping)?|eras(?:e|es|ing)|destroy(?:s|ing)?|eliminat(?:e|es|ing)|nuk(?:e|es|ing)|rm(?:\s+-rf?)?)";

const FORBIDDEN_REMOVAL_DIRS: &[&str] = &[
    "crates", "bins", "apps", "algorithms", "mirror", "drones", "scripts",
    "lib", "substrate", "ui", "zig_muscle",
    "comparison_workspace", "fuzz", "genesis_lab", "target_dummy",
    ".github", ".cargo", "config", "docs", "tests",
    "vault", "data", "assets", "contrib",
    "vendor", "third_party",
];

struct ScanRegexes {
    pat: Regex,
    neg: Regex,
    hist: Regex,
}

fn scan_regexes() -> &'static ScanRegexes {
    static S: OnceLock<ScanRegexes> = OnceLock::new();
    S.get_or_init(|| {
        // Longest-first so "comparison_workspace" etc. win over shorter prefixes.
        let mut dirs: Vec<&str> = FORBIDDEN_REMOVAL_DIRS.to_vec();
        dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
        let alt = dirs
            .iter()
            .map(|d| regex::escape(d))
            .collect::<Vec<_>>()
            .join("|");
        // Rust's regex has no lookbehind/lookahead: boundary checks are done
        // manually in halt_recommends_forbidden_removal().
        let pat = format!(r"(?:{VERBS})\s+(?:the\s+)?`?(?:{alt})`?");
        ScanRegexes {
            pat: Regex::new(&pat).expect("pat regex"),
            neg: Regex::new(
                r"\b(?:not|never|no|without|avoid|against|unless|don't|doesn't|shouldn't)\b|n't",
            )
            .expect("neg regex"),
            hist: Regex::new(
                r"\b(?:said|says|recommended|suggested|discussed|proposed|previous|prior|last|earlier|former|old)\b",
            )
            .expect("hist regex"),
        }
    })
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Split on ./!/? followed by whitespace, or on '\n' / ';'.
/// ".github" and "lib/" survive because no whitespace follows the dot.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        if c == '\n' || c == ';' {
            out.push(&text[start..i]);
            start = i + c.len_utf8();
        } else if c.is_whitespace() && matches!(prev, Some('.' | '!' | '?')) {
            out.push(&text[start..i]);
            start = i + c.len_utf8();
        }
        prev = Some(c);
    }
    out.push(&text[start..]);
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

/// True if `prefix` contains an opening quote (not an in-word apostrophe)
/// that is never closed before the end of the prefix.
fn has_unclosed_open_quote(prefix: &str) -> bool {
    let chars: Vec<char> = prefix.chars().collect();
    for i in 0..chars.len() {
        if !matches!(chars[i], '\'' | '"' | '\u{201C}' | '\u{2018}') {
            continue;
        }
        if i > 0 && is_word(chars[i - 1]) {
            continue; // apostrophe inside a word, e.g. "donald's"
        }
        if chars[i + 1..]
            .iter()
            .all(|c| !matches!(c, '\'' | '"' | '\u{2019}' | '\u{201D}'))
        {
            return true;
        }
    }
    false
}

/// Returns Some(matched text) if the reasoning actively recommends removing a
/// protected directory; None otherwise.
pub fn halt_recommends_forbidden_removal(reasoning: &str) -> Option<String> {
    let s = scan_regexes();
    let lower = reasoning.to_lowercase();
    for sent in split_sentences(&lower) {
        for m in s.pat.find_iter(sent) {
            // Boundary before the verb (replaces (?<![\w.-]))
            if let Some(b) = sent[..m.start()].chars().next_back() {
                if is_word(b) || b == '.' || b == '-' {
                    continue;
                }
            }
            // Boundary after the dir (replaces the lookahead)
            if let Some(a) = sent[m.end()..].chars().next() {
                if !(a.is_whitespace()
                    || matches!(a, '/' | ',' | '.' | ';' | ':' | ')' | '`'))
                {
                    continue;
                }
            }
            let prefix = &sent[..m.start()];
            if s.neg.is_match(prefix)
                || s.hist.is_match(prefix)
                || has_unclosed_open_quote(prefix)
            {
                continue;
            }
            return Some(m.as_str().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // (class, label, text, should_flag)
    fn cases() -> Vec<(&'static str, &'static str, &'static str, bool)> {
        vec![
            ("1", "recommendation: two dirs", "Recommendation: removing the config directory and the tests directory", true),
            ("1", "should delete vendor", "We should delete the vendor/ tree", true),
            ("1", "cleanup plan .github", "Cleanup plan: remove the .github directory", true),
            ("1", "rm -rf form", "Next step: rm -rf vendor/", true),
            ("1", "negation then imperative", "Do not touch crates; delete the tests directory", true),
            ("1", "multi-sentence", "The audit is done. Remove the substrate directory.", true),
            ("2", "description: config", "The config directory has 3 subdirectories.", false),
            ("2", "description: tests", "The tests directory was examined and found healthy.", false),
            ("2", "description: vendor", "The vendor/ tree was inspected for licenses.", false),
            ("3", "short negation", "Do not delete the tests directory", false),
            ("3", "long negation", "Under no circumstances should anyone remove the crates directory", false),
            ("3", "long negation 2", "We should never under any circumstance consider removing the config directory", false),
            ("3", "avoid", "Avoid removing the vault directory at all costs", false),
            ("4", "quoted prior audit", "The prior audit said 'removing the lib directory is a good idea'", false),
            ("4", "double-quoted", r#"He wrote "remove the docs directory" in the old report"#, false),
            ("4", "past tense recommended", "A previous report recommended removing the config directory", false),
            ("4", "historical discussion", "Last week we discussed removing the tests directory", false),
            ("5", "substring: perform", "We perform the tests every night", false),
            ("5", "substring: confirm", "Please confirm the docs are current", false),
            ("5", "substring: backdrop", "A backdrop the config page uses", false),
            ("5", "apostrophe not quote", "Donald's plan: remove the config directory", true),
        ]
    }

    #[test]
    fn semantics_matrix() {
        let mut failures = Vec::new();
        for (cls, label, text, should_flag) in cases() {
            let got = halt_recommends_forbidden_removal(text).is_some();
            if got != should_flag {
                failures.push(format!(
                    "class {cls} [{label}]: expected flag={should_flag}, got {got}: {text}"
                ));
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    #[test]
    fn flags_the_2026_11_14_failed_audit_report() {
        // Exact bytes from the live run that recommended deleting protected trees.
        let report = include_str!("../../tests/fixtures/failed_audit_report_2026_11_14.md");
        let hit = halt_recommends_forbidden_removal(report);
        assert!(hit.is_some(), "the failed report must be flagged by the removal scan");
    }

    #[test]
    fn does_not_flag_a_plain_description_of_the_same_dirs() {
        let ok = "The vendor/ tree and the config directory were inspected. \
                  The tests directory has 12 files. Nothing should be removed.";
        assert!(halt_recommends_forbidden_removal(ok).is_none());
    }
}
