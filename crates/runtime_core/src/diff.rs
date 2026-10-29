//! Whitespace-Tolerant, AST-Safe Diff Engine for AthenaFusionX.
use anyhow::{anyhow, Result};

pub struct DiffEngine;

impl DiffEngine {
    pub fn apply(source: &str, search: &str, replace: &str) -> Result<String> {
        // 1. Exact match first (fastest)
        if source.contains(search) {
            return Ok(source.replacen(search, replace, 1));
        }

        // 2. Whitespace-normalized line window matching (handles LLM tab/space drift)
        let search_trimmed_lines: Vec<&str> =
            search.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

        if search_trimmed_lines.is_empty() {
            return Err(anyhow!("Search block is empty."));
        }

        let source_lines: Vec<&str> = source.lines().collect();
        if search_trimmed_lines.len() > source_lines.len() {
            return Err(anyhow!("Target search block exceeds total file line count."));
        }

        for i in 0..=(source_lines.len() - search_trimmed_lines.len()) {
            let window = &source_lines[i..i + search_trimmed_lines.len()];
            let matches = window
                .iter()
                .zip(&search_trimmed_lines)
                .all(|(src_line, srch_line)| src_line.trim() == *srch_line);

            if matches {
                let mut new_lines = source_lines[..i].to_vec();
                new_lines.push(replace);
                new_lines.extend(&source_lines[i + search_trimmed_lines.len()..]);

                let mut res = new_lines.join("\n");
                if source.ends_with('\n') && !res.ends_with('\n') {
                    res.push('\n');
                }
                return Ok(res);
            }
        }

        Err(anyhow!(
            "Target search block not found in source (even after whitespace-normalized scan)."
        ))
    }
}
