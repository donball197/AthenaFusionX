//! Evidence-derived coverage tracking.
//!
//! Coverage is counted ONLY from physical evidence markers recorded by the
//! executor. Model-authored claims (e.g. "Lines read so far: 1-100") are
//! never ingested.
#![allow(dead_code)]

use std::collections::BTreeSet;

const CHUNK_MARKER: &str = "[READ_CHUNK_OK]";
const READ_OK_MARKER: &str = "[READ_OK]";
const TRUNCATION_MARKER: &str = "[CONTENT TRUNCATED TO SAVE TOKENS.";

#[derive(Debug, Default, Clone)]
pub struct CoverageLedger {
    covered: BTreeSet<u32>,
    total_lines: Option<u32>,
    /// The file this ledger currently describes. Reading a different file resets
    /// the ledger: line numbers from one file say nothing about another, and the
    /// percentage is relative to the latest file's own total.
    current_path: Option<String>,
}

impl CoverageLedger {
    /// Idempotent: replayed evidence (reflection blocks repeat markers) is safe.
    pub fn ingest(&mut self, evidence: &str) {
        let lines: Vec<&str> = evidence.lines().collect();

        // 1. Find the last path actively read in the evidence
        let mut target_path = None;
        for line in &lines {
            if line.starts_with("[READ_CHUNK_OK]") {
                if let Some(p) = chunk_line_path(line) {
                    target_path = Some(p);
                }
            } else if line.starts_with("[READ_OK] ") {
                let p = line.trim_start_matches("[READ_OK] ").trim();
                target_path = Some(p.to_string());
            }
        }

        let Some(target) = target_path else {
            return;
        };
        self.ingest_target(&lines, &target);
    }

    pub fn ingest_all_files(evidence: &str) -> Vec<Self> {
        let lines: Vec<&str> = evidence.lines().collect();
        let mut seen: Vec<String> = Vec::new();
        for line in &lines {
            let raw = if line.starts_with("[READ_CHUNK_OK]") {
                chunk_line_path(line)
            } else if line.starts_with("[READ_OK] ") {
                Some(line.trim_start_matches("[READ_OK] ").trim().to_string())
            } else {
                None
            };
            if let Some(p) = raw {
                let norm = normalize_workspace_path(&p);
                if !norm.is_empty() && !seen.contains(&norm) {
                    seen.push(norm);
                }
            }
        }
        seen.into_iter()
            .map(|t| {
                let mut l = Self::default();
                l.ingest_target(&lines, &t);
                l
            })
            .collect()
    }

    fn ingest_target(&mut self, lines: &[&str], target: &str) {
        let target_norm = normalize_workspace_path(target);
        // 2. Process all events chronologically, accumulating coverage for the target path
        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("[READ_CHUNK_OK]") {
                if let Some(p) = chunk_line_path(lines[i]) {
                    if normalize_workspace_path(&p) == target_norm {
                        if let Some((s, e, total)) = parse_chunk_line(lines[i]) {
                            self.switch_to(&p);
                            self.total_lines = Some(total);
                            for l in s..=e.min(total) {
                                self.covered.insert(l);
                            }
                        }
                    }
                }
                i += 1;
            } else if lines[i].starts_with("[READ_OK] ") {
                let p = lines[i].trim_start_matches("[READ_OK] ").trim();
                let mut j = i + 1;
                while j < lines.len()
                    && !lines[j].starts_with("[READ_CHUNK_OK]")
                    && !lines[j].starts_with("[READ_OK]")
                {
                    j += 1;
                }
                if normalize_workspace_path(p) == target_norm {
                    let slice = lines[i..j].join(
                        "
",
                    );
                    self.ingest_read_ok_blocks(&slice);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }

    fn ingest_read_ok_blocks(&mut self, evidence: &str) {
        let lines: Vec<&str> = evidence.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if !lines[i].starts_with(READ_OK_MARKER) {
                i += 1;
                continue;
            }
            // Skip to the FULL FILE CONTENT: header if present.
            let mut j = i + 1;
            while j < lines.len() && !lines[j].starts_with("FULL FILE CONTENT:") {
                if lines[j].starts_with('[') {
                    break;
                } // no content block
                j += 1;
            }
            if j >= lines.len() || !lines[j].starts_with("FULL FILE CONTENT:") {
                i += 1;
                continue;
            }
            if let Some(p) = lines[i][READ_OK_MARKER.len()..].split_whitespace().next() {
                self.switch_to(p);
            }
            // Count content lines until the next evidence marker or truncation note.
            let mut shown: u32 = 0;
            let mut total: Option<u32> = None;
            let mut k = j + 1;
            while k < lines.len() {
                let l = lines[k];
                if l.starts_with(TRUNCATION_MARKER) {
                    total = parse_total_lines(l);
                    break;
                }
                if is_read_block_terminator(l) {
                    break;
                } // next evidence marker
                shown += 1;
                k += 1;
            }
            if let Some(total) = total {
                self.total_lines = Some(total);
                // Only the *shown* prefix is proven read; the truncation marker
                // explicitly says the rest was not provided.
                for l in 1..=shown.min(total) {
                    self.covered.insert(l);
                }
            } else {
                // Untruncated full read: the whole file is proven read.
                let total = shown;
                self.total_lines = Some(total);
                for l in 1..=total {
                    self.covered.insert(l);
                }
            }
            i = k;
        }
    }
}

fn is_read_block_terminator(line: &str) -> bool {
    const RECORD_PREFIXES: &[&str] = &[
        "[READ_OK]",
        "[READ_CHUNK_OK]",
        "[READ_FUNCTION_OK]",
        "[WRITE_OK]",
        "[WRITE_NOOP]",
        "[CMD_OK]",
        "[SEARCH_CODE_OK]",
        "[SEARCH_CHECKED_OK]",
        "[SEARCH_MEMORY_OK]",
        "[PATCH_FUNCTION_OK]",
        "[PATCH_IDEMPOTENT_OK]",
        "[DIFF_PATCH_OK]",
        "[REPLACE_LINES_OK]",
        "[ACTION FAILED]",
        "[HALT]",
        "[HALT_REJECTED",
        "[DAG LINEAGE",
        "[PROPOSITION]",
        "[CRITIC_VETO]",
        "[AUTO_VERIFY_READ]",
        "[NODE PHASE]",
    ];
    let t = line.trim_start();
    RECORD_PREFIXES.iter().any(|p| t.starts_with(p))
}

impl CoverageLedger {
    pub fn coverage_pct(&self) -> f32 {
        match self.total_lines {
            Some(t) if t > 0 => 100.0 * self.covered.len() as f32 / t as f32,
            _ => 0.0,
        }
    }

    pub fn total_lines(&self) -> Option<u32> {
        self.total_lines
    }

    /// Widen `best` to (s,e) when (s,e) is strictly wider.
    /// Length math is done on the *new* run only — never subtracting a later
    /// start from an earlier end (that underflows u32).
    fn widen(best: &mut Option<(u32, u32)>, s: u32, e: u32) {
        let wider = match *best {
            None => true,
            Some((bs, be)) => (e - s) > (be - bs),
        };
        if wider {
            *best = Some((s, e));
        }
    }

    /// Largest contiguous hole in 1..=total. None when fully covered.
    pub fn largest_gap(&self) -> Option<(u32, u32)> {
        let total = self.total_lines?;
        let mut best: Option<(u32, u32)> = None;
        let mut run_start: Option<u32> = None;
        for l in 1..=total {
            if !self.covered.contains(&l) {
                run_start.get_or_insert(l);
            } else if let Some(s) = run_start.take() {
                Self::widen(&mut best, s, l - 1);
            }
        }
        if let Some(s) = run_start {
            Self::widen(&mut best, s, total);
        }
        best
    }

    /// Largest gap strictly *between* two covered regions — the silent hole:
    /// content that looks contiguous from the outside but was never read.
    /// The trailing unread tail is deliberately excluded (see
    /// `largest_gap` for that).
    pub fn largest_interior_gap(&self) -> Option<(u32, u32)> {
        let total = self.total_lines?;
        let mut best: Option<(u32, u32)> = None;
        let mut run_start: Option<u32> = None;
        let mut seen_covered = false;
        for l in 1..=total {
            if !self.covered.contains(&l) {
                run_start.get_or_insert(l);
            } else {
                if let Some(s) = run_start.take() {
                    if seen_covered {
                        Self::widen(&mut best, s, l - 1);
                    }
                }
                seen_covered = true;
            }
        }
        best
    }

    pub fn covers(&self, start: u32, end: u32) -> bool {
        (start..=end).all(|l| self.covered.contains(&l))
    }
}

/// Parses: `[READ_CHUNK_OK] File: <path> | Lines 100-150 of 2415`
pub fn parse_chunk_line(line: &str) -> Option<(u32, u32, u32)> {
    if !line.contains(CHUNK_MARKER) {
        return None;
    }
    let idx = line.find("Lines ")? + "Lines ".len();
    let (s, rest) = parse_u32(&line[idx..])?;
    let (e, rest) = parse_u32(rest.strip_prefix('-')?)?;
    let (total, _) = parse_u32(rest.trim_start().strip_prefix("of")?.trim_start())?;
    if s >= 1 && s <= e && e <= total {
        Some((s, e, total))
    } else {
        None // malformed or out-of-range marker: refuse to trust it
    }
}

fn parse_u32(s: &str) -> Option<(u32, &str)> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    Some((digits.parse().ok()?, &s[digits.len()..]))
}

fn parse_total_lines(truncation_line: &str) -> Option<u32> {
    let idx = truncation_line.find("TOTAL LINES:")? + "TOTAL LINES:".len();
    parse_u32(truncation_line[idx..].trim_start()).map(|(n, _)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Literal lines from the runs we audited.
    const CHUNK_100_150: &str =
        "[READ_CHUNK_OK] File: crates/runtime_core/src/cortical_loop.rs | Lines 100-150 of 2415";

    #[test]
    fn parses_real_log_marker() {
        assert_eq!(parse_chunk_line(CHUNK_100_150), Some((100, 150, 2415)));
    }

    #[test]
    fn model_claims_are_ignored() {
        let mut l = CoverageLedger::default();
        l.ingest("Lines read so far: 1-100 / Total 2415. Coverage: 4.1%");
        assert_eq!(l.coverage_pct(), 0.0);
    }

    /// The exact bug from the last audit run: lines 52-99 never read.
    #[test]
    fn detects_the_52_99_gap() {
        let mut l = CoverageLedger::default();
        // Truncated full read: 51 content lines shown before the truncation
        // note, then the only chunk read was 100-150.
        let mut full = String::from(
            "[READ_OK] crates/runtime_core/src/cortical_loop.rs\nFULL FILE CONTENT:\n",
        );
        for i in 1..=51 {
            full.push_str(&format!("line {i}\n"));
        }
        full.push_str("[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: 2415. USE THE `read_file_chunk` TOOL]\n");
        l.ingest(&full);
        l.ingest(CHUNK_100_150);

        // The silent interior hole is the audit target ...
        assert_eq!(l.largest_interior_gap(), Some((52, 99)));
        assert!(!l.covers(52, 99));
        assert!(l.covers(100, 150));
        // ... while the widest hole overall is the unread tail. Both must be
        // computable without underflow.
        assert_eq!(l.largest_gap(), Some((151, 2415)));
    }

    #[test]
    fn replay_is_idempotent() {
        let mut l = CoverageLedger::default();
        l.ingest(CHUNK_100_150);
        l.ingest(CHUNK_100_150); // reflection block replay
        assert_eq!(l.covered.len(), 51);
    }

    #[test]
    fn rejects_malformed_marker() {
        assert_eq!(parse_chunk_line("[READ_CHUNK_OK] Lines 5-1 of 10"), None); // s > e
        assert_eq!(parse_chunk_line("[READ_CHUNK_OK] Lines 1-99 of 10"), None); // e > total
    }
}

impl CoverageLedger {
    /// Point the ledger at `path`, clearing it when the file changes.
    fn switch_to(&mut self, path: &str) {
        let p = normalize_workspace_path(path);
        if p.is_empty() {
            return;
        }
        if self.current_path.as_deref() != Some(p.as_str()) {
            self.covered.clear();
            self.total_lines = None;
            self.current_path = Some(p);
        }
    }
}

/// Absolute and cwd-relative spellings of one workspace file are the same file.
fn normalize_workspace_path(p: &str) -> String {
    let p = p.trim();
    std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            std::path::Path::new(p).strip_prefix(&cwd).ok().map(|r| r.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| p.trim_start_matches("./").to_string())
}

/// Path of a `[READ_CHUNK_OK] File: <path> | Lines S-E of N` marker line.
fn chunk_line_path(line: &str) -> Option<String> {
    let rest = line.split("File:").nth(1)?;
    let path = rest.split('|').next()?.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

#[cfg(test)]
mod per_file_tests {
    use super::CoverageLedger;

    fn read_ok(path: &str, lines: usize) -> String {
        let mut s = format!("[READ_OK] {}\nFULL FILE CONTENT:\n", path);
        for i in 0..lines {
            s.push_str(&format!("line {}\n", i));
        }
        s
    }

    #[test]
    fn reading_a_second_file_does_not_inherit_the_first_files_lines() {
        let mut l = CoverageLedger::default();
        l.ingest(&read_ok("a.rs", 96));
        l.ingest(&read_ok("b.rs", 47));
        assert_eq!(l.total_lines(), Some(47));
        assert!((l.coverage_pct() - 100.0).abs() < 0.01, "{}", l.coverage_pct());
    }

    #[test]
    fn a_truncated_read_after_a_longer_file_reports_its_own_gap() {
        let mut l = CoverageLedger::default();
        l.ingest(&read_ok("a.rs", 200));
        l.ingest(
            "[READ_OK] b.rs\nFULL FILE CONTENT:\nl1\nl2\nl3\n[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: 10. USE THE `read_file_chunk` TOOL BY SPECIFYING `start_line` AND `max_lines` TO READ BEYOND THIS POINT.]\n",
        );
        assert_eq!(l.total_lines(), Some(10));
        assert_eq!(l.largest_gap(), Some((4, 10)));
    }

    #[test]
    fn absolute_and_relative_spellings_are_the_same_file() {
        let abs = std::env::current_dir().unwrap().join("x/y.rs").to_string_lossy().to_string();
        let mut l = CoverageLedger::default();
        l.ingest("[READ_CHUNK_OK] File: x/y.rs | Lines 1-50 of 100\nCONTENT:\nz\n");
        l.ingest(&format!("[READ_CHUNK_OK] File: {} | Lines 51-100 of 100\nCONTENT:\nz\n", abs));
        assert!((l.coverage_pct() - 100.0).abs() < 0.01, "{}", l.coverage_pct());
    }
}

#[cfg(test)]
mod embedded_tag_truncation_tests {
    use super::CoverageLedger;

    #[test]
    fn truncated_read_with_embedded_bracket_tag_still_reports_gap() {
        let mut ev =
            String::from("[READ_OK] crates/runtime_core/src/strategize.rs\nFULL FILE CONTENT:\n");
        for i in 1..=10 {
            ev.push_str(&format!("// line {}\n", i));
        }
        ev.push_str("[PARSE_FAULT_ACTION] embedded string literal inside source file\n");
        for i in 12..=50 {
            ev.push_str(&format!("// line {}\n", i));
        }
        ev.push_str(
            "[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: 100. USE THE `read_file_chunk` TOOL]\n",
        );
        let mut ledger = CoverageLedger::default();
        ledger.ingest(&ev);
        assert_eq!(ledger.total_lines(), Some(100));
        assert!(ledger.coverage_pct() < 100.0, "got {}", ledger.coverage_pct());
        assert_eq!(ledger.largest_gap(), Some((51, 100)));
    }
}
