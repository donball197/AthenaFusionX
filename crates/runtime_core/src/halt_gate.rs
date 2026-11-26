//! Deterministic gate: a halt may not claim line ranges that were never read.
//!
//! Bare `fn_name` citations (backticks only, no explicit "lines X-Y") are
//! accepted when the name appears in the READ_* verification corpus.
//! Only *explicit* line ranges are checked against the coverage ledger.
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
    let lower = text.to_lowercase();
    if !lower.contains("line") {
        return Vec::new();
    }
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
                        if a >= 1 && b >= a && preceded_by_line_word(&chars, s0) {
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

/// True when the number starting at `s0` is introduced by the word "line"/"lines"
/// (optionally after whitespace, '(' or ':'). `file.rs:15-38` and "149 LINES"
/// are not line-range claims; the file-aware outline check covers those.
fn preceded_by_line_word(chars: &[char], s0: usize) -> bool {
    let mut k = s0;
    while k > 0 && (chars[k - 1].is_whitespace() || chars[k - 1] == '(' || chars[k - 1] == ':') {
        k -= 1;
    }
    let head: String = chars[..k].iter().collect::<String>().to_lowercase();
    let w = if head.ends_with("lines") {
        5
    } else if head.ends_with("line") {
        4
    } else {
        return false;
    };
    head[..head.len() - w].chars().next_back().map_or(true, |c| !c.is_alphanumeric() && c != '_')
}

/// None when the halt is acceptable; Some(reason) when it must be rejected.
pub fn halt_range_gate(evidence: &str) -> Option<String> {
    let cited = cited_ranges(evidence);
    if cited.is_empty() {
        // No explicit lines X-Y claims → range gate passes.
        // Cite bare `fn_name` tokens; do not invent line ranges.
        return None;
    }
    let ledgers = CoverageLedger::ingest_all_files(evidence);
    let with_totals: Vec<&CoverageLedger> =
        ledgers.iter().filter(|l| l.total_lines().is_some()).collect();
    if with_totals.is_empty() {
        return None;
    }
    let max_total = with_totals.iter().filter_map(|l| l.total_lines()).max().unwrap_or(0);
    for (s, e) in cited {
        if e > max_total {
            return Some(format!(
                "halt cites lines {s}-{e} but the file has only {max_total} lines"
            ));
        }
        if !with_totals.iter().any(|l| l.total_lines().is_some_and(|t| e <= t) && l.covers(s, e)) {
            return Some(format!("halt cites lines {s}-{e} that were never fully read"));
        }
    }
    None
}

/// Names cited only as `ident` (no "lines X-Y" clause).
#[allow(dead_code)]
fn bare_backticked_names(halt_body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = halt_body;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            let name = rest[..end].trim();
            // skip empty, skip span-like "12-40", skip paths
            if !name.is_empty()
                && !name.contains('/')
                && !name.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
            {
                out.push(name.to_string());
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    out
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
    fn file_colon_ranges_and_uppercase_lines_word_are_not_claims() {
        let ev = format!(
            "{}[HALT] FINAL REPORT (149 LINES): `f` (x.rs:430-450) and (y.rs:15-38) ok.\n",
            read_all(100)
        );
        assert!(halt_range_gate(&ev).is_none());
    }

    #[test]
    fn inline_and_outline_words_do_not_arm_the_gate() {
        assert!(ranges_in("inline report 0.65-0.75 and outline 15-38").is_empty());
    }

    #[test]
    fn identifiers_with_digits_are_not_ranges() {
        assert!(ranges_in("see v2-3 and utf8-16").is_empty());
    }

    #[test]
    fn multifile_audit_accepts_earlier_longer_file_range() {
        let ev = concat!(
            "[READ_CHUNK_OK] File: task_manager.rs | Lines 1-300 of 300\nCONTENT:\na\n",
            "[READ_OK] scheduler.rs\nFULL FILE CONTENT:\nb\nc\n",
            "[HALT] task_manager.rs lines 150-250 gate completion.\n",
        );
        assert!(halt_range_gate(ev).is_none());
        let bad = concat!(
            "[READ_CHUNK_OK] File: task_manager.rs | Lines 1-100 of 300\nCONTENT:\na\n",
            "[READ_OK] scheduler.rs\nFULL FILE CONTENT:\nb\nc\n",
            "[HALT] task_manager.rs lines 150-250 gate completion.\n",
        );
        assert!(halt_range_gate(bad).is_some());
    }
}
