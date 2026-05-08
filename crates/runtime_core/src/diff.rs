/**
 * NEXUS Diff Engine — v2.0 [SURGICAL + FUZZY]
 * Copyright 2026 Donald Ball
 * Handles precise edits even when LLM mangles whitespace or line order.
 */
use anyhow::{anyhow, Result};

pub struct DiffEngine;

impl DiffEngine {
    /// Primary entry point: tries exact match first, then fuzzy/indentation-agnostic.
    pub fn apply(original: &str, search: &str, replace: &str) -> Result<String> {
        if search.trim().is_empty() {
            return Ok(original.to_string());
        }

        // Tier 1: Exact match (fastest, most precise)
        if let Some(pos) = original.find(search) {
            return Ok(replace_first_occurrence(original, pos, search.len(), replace));
        }

        // Tier 2: Indentation-agnostic matching (handles LLM whitespace issues)
        if let Some(result) = Self::apply_agnostic(original, search, replace) {
            return Ok(result);
        }

        // Tier 3: Fuzzy line-based matching (for larger blocks)
        if let Some(result) = Self::apply_fuzzy_lines(original, search, replace) {
            return Ok(result);
        }

        Err(anyhow!(
            "PATTERN_NOT_FOUND: Could not locate search block.\n\
            Search preview (first 120 chars):\n{}\n\n\
            Hint: Try making the search string more unique or shorter.",
            search.chars().take(120).collect::<String>()
        ))
    }

    fn apply_agnostic(original: &str, search: &str, replace: &str) -> Option<String> {
        let search_lines: Vec<&str> = search.lines().map(|l| l.trim_start()).collect();
        let orig_lines: Vec<&str> = original.lines().collect();
        let search_len = search_lines.len();

        if search_len == 0 || orig_lines.len() < search_len {
            return None;
        }

        for i in 0..=(orig_lines.len() - search_len) {
            let window = &orig_lines[i..i + search_len];
            let window_trimmed: Vec<&str> = window.iter().map(|l| l.trim_start()).collect();

            if window_trimmed == search_lines {
                let mut result = orig_lines[..i].join("\n");
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(replace);
                if i + search_len < orig_lines.len() {
                    result.push('\n');
                    result.push_str(&orig_lines[i + search_len..].join("\n"));
                }
                return Some(result);
            }
        }
        None
    }

    fn apply_fuzzy_lines(original: &str, search: &str, replace: &str) -> Option<String> {
        // Very coarse fallback: if search is long, try to match by first and last line
        let search_lines: Vec<&str> = search.lines().collect();
        if search_lines.len() < 3 {
            return None;
        }

        let first = search_lines[0].trim();
        let last = search_lines.last().unwrap().trim();

        let orig_lines: Vec<&str> = original.lines().collect();

        for i in 0..orig_lines.len() {
            if orig_lines[i].trim() == first {
                for j in (i + 2)..orig_lines.len() {
                    if orig_lines[j].trim() == last {
                        let mut result = orig_lines[..i].join("\n");
                        if !result.is_empty() {
                            result.push('\n');
                        }
                        result.push_str(replace);
                        if j + 1 < orig_lines.len() {
                            result.push('\n');
                            result.push_str(&orig_lines[j + 1..].join("\n"));
                        }
                        return Some(result);
                    }
                }
            }
        }
        None
    }

    pub fn unified_diff(_old: &str, _new: &str, path: &str) -> String {
        format!("--- a/{}\n+++ b/{}\n[MODIFICATION APPLIED]", path, path)
    }
}

fn replace_first_occurrence(s: &str, pos: usize, old_len: usize, new: &str) -> String {
    let mut result = String::with_capacity(s.len() - old_len + new.len());
    result.push_str(&s[..pos]);
    result.push_str(new);
    result.push_str(&s[pos + old_len..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let src = "fn old() {}\nfn keep() {}";
        let result = DiffEngine::apply(src, "fn old() {}", "fn new() {}").unwrap();
        assert_eq!(result, "fn new() {}\nfn keep() {}");
    }

    #[test]
    fn test_agnostic_whitespace() {
        let src = "    fn foo() {\n        println!(\"hi\");\n    }";
        let search = "fn foo() {\n    println!(\"hi\");\n}";
        let replace = "fn bar() {\n    println!(\"hello\");\n}";
        let result = DiffEngine::apply(src, search, replace).unwrap();
        assert!(result.contains("fn bar()"));
    }
}
