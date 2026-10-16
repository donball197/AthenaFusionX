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
}

impl CoverageLedger {
    /// Idempotent: replayed evidence (reflection blocks repeat markers) is safe.
    pub fn ingest(&mut self, evidence: &str) {
        for line in evidence.lines() {
            if let Some((s, e, total)) = parse_chunk_line(line) {
                self.total_lines = Some(total);
                for l in s..=e.min(total) {
                    self.covered.insert(l);
                }
            }
        }
        self.ingest_read_ok_blocks(evidence);
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
                if lines[j].starts_with('[') { break; } // no content block
                j += 1;
            }
            if j >= lines.len() || !lines[j].starts_with("FULL FILE CONTENT:") {
                i += 1;
                continue;
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
                if l.starts_with('[') && l.contains("] ") { break; } // next marker
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
        let mut full = String::from("[READ_OK] crates/runtime_core/src/cortical_loop.rs\nFULL FILE CONTENT:\n");
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
