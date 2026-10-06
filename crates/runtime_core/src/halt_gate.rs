//! Deterministic gate: a halt may not claim line ranges that were never read.
//!
//! Rebuilds the read-coverage ledger from the evidence text and checks every
//! line range the halt body cites. A halt that cites no range passes; this
//! gate rejects false claims, it does not demand detail.

use crate::coverage::CoverageLedger;

/// Line ranges (start, end), 1-based inclusive, cited in `[HALT]` bodies.
pub fn cited_ranges(evidence: &str) -> Vec<(u32, u32)> {
    // Only the LAST halt block is judged: earlier (rejected) halts stay in the
    // evidence buffer and must not block a corrected one. Any other record
    // marker, including [HALT_REJECTED], ends the block.
    let mut out: Vec<(u32, u32)> = Vec::new();
    let mut in_halt = false;
    for line in evidence.lines() {
        let t = line.trim_start();
        if t.starts_with("[HALT]") {
            in_halt = true;
            out.clear();
        } else if t.starts_with('[') {
            in_halt = false;
        }
        if in_halt {
            out.extend(ranges_in(t));
        }
    }
    out
}

/// Parses "lines 1-806", "Lines 201–648", "(150-350)" style ranges.
fn ranges_in(text: &str) -> Vec<(u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_ascii_alphanumeric()) {
            let s0 = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let a: String = chars[s0..i].iter().collect();
            if i < chars.len() && (chars[i] == '-' || chars[i] == '\u{2013}') {
                let j0 = i + 1;
                let mut j = j0;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j > j0 && !(j < chars.len() && chars[j].is_ascii_alphabetic()) {
                    let b: String = chars[j0..j].iter().collect();
                    if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                        if a >= 1 && b >= a {
                            out.push((a, b));
                        }
                    }
                    i = j;
                    continue;
                }
            }
        } else {
            i += 1;
            continue;
        }
    }
    out
}

/// None when the halt is acceptable; Some(reason) when it must be rejected.
pub fn halt_range_gate(evidence: &str) -> Option<String> {
    let cited = cited_ranges(evidence);
    if cited.is_empty() {
        return None;
    }
    let mut ledger = CoverageLedger::default();
    ledger.ingest(evidence);
    let total = ledger.total_lines()?;
    for (s, e) in cited {
        // A range beyond the file is a false claim by definition.
        if e > total {
            return Some(format!(
                "halt cites lines {s}-{e} but the file has only {total} lines"
            ));
        }
        if !ledger.covers(s, e) {
            return Some(format!("halt cites lines {s}-{e} that were never fully read"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_all(total: u32) -> String {
        format!("[READ_CHUNK_OK] File: x.rs | Lines 1-{total} of {total}\nCONTENT:\nbody\n")
    }

    #[test]
    fn halt_without_ranges_passes() {
        let ev = format!("{}[HALT] The loop has a state machine.\n", read_all(100));
        assert!(halt_range_gate(&ev).is_none());
    }

    #[test]
    fn halt_citing_read_range_passes() {
        let ev = format!("{}[HALT] ignite (Lines 10-60) drives it.\n", read_all(100));
        assert!(halt_range_gate(&ev).is_none());
    }

    #[test]
    fn halt_citing_unread_range_is_rejected() {
        let ev = "[READ_CHUNK_OK] File: x.rs | Lines 1-50 of 200\nCONTENT:\nb\n\
                  [HALT] handle_execute (lines 120-180) writes files.\n";
        let why = halt_range_gate(ev).expect("must reject");
        assert!(why.contains("120-180"), "{why}");
    }

    #[test]
    fn range_past_end_of_file_is_rejected() {
        let ev = format!("{}[HALT] see lines 90-400.\n", read_all(100));
        assert!(halt_range_gate(&ev).is_some());
    }

    #[test]
    fn ranges_in_read_content_are_not_halt_claims() {
        let ev = "[READ_CHUNK_OK] File: x.rs | Lines 1-50 of 200\nCONTENT:\n// see lines 150-190\n\
                  [HALT] done.\n";
        assert!(halt_range_gate(ev).is_none());
    }

    #[test]
    fn multiline_halt_body_is_scanned() {
        let ev = "[READ_CHUNK_OK] File: x.rs | Lines 1-50 of 200\nCONTENT:\nb\n\
                  [HALT] SUMMARY:\n- foo (Lines 100-140)\n";
        assert!(halt_range_gate(ev).is_some());
    }

    #[test]
    fn earlier_bad_halt_does_not_block_a_corrected_one() {
        let ev = "[READ_CHUNK_OK] File: x.rs | Lines 1-50 of 200\nCONTENT:\nb\n\
                  [HALT] bad claim lines 100-140\n\
                  [HALT_REJECTED] nope\n\
                  [HALT] fixed, see lines 10-40\n";
        assert!(halt_range_gate(ev).is_none());
    }

    #[test]
    fn rejection_text_is_not_a_claim() {
        let ev = "[READ_CHUNK_OK] File: x.rs | Lines 1-50 of 200\nCONTENT:\nb\n\
                  [HALT] summary without ranges\n\
                  [HALT_REJECTED] actually lines 150-190 contain the loop\n";
        assert!(halt_range_gate(ev).is_none());
    }

    #[test]
    fn identifiers_with_digits_are_not_ranges() {
        assert!(ranges_in("see v2-3 and utf8-16").is_empty());
    }
}
