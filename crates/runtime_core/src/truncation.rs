//! Executor-side completion of truncated `read_file` output.
//!
//! `read_file` caps output and appends:
//!   `[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: N. USE THE `read_file_chunk` ...]`
//! Downstream components (strategist, critic, coverage ledger) then disagree about
//! what is unread. This module lets the executor finish the read itself so the
//! model never sees a gap.

use anyhow::Result;

const MARKER_PREFIX: &str = "[CONTENT TRUNCATED TO SAVE TOKENS.";
const TOTAL_KEY: &str = "TOTAL LINES:";
const CONTENT_HEADER: &str = "FULL FILE CONTENT:";
/// Lines requested per follow-up chunk. Keep below the executor's char cap.
pub const CHUNK_LINES: u32 = 80;
/// Hard stop so a pathological file cannot spin the executor.
pub const MAX_CHUNKS: usize = 24;

#[derive(Debug, PartialEq, Eq)]
pub struct Truncation {
    /// 1-based line to resume from (re-reads a partially returned line).
    pub resume_line: u32,
    pub total_lines: u32,
}

/// Detects the truncation marker and works out where to resume.
pub fn detect(output: &str) -> Option<Truncation> {
    let m = output.find(MARKER_PREFIX)?;
    let marker = &output[m..];
    let t = marker.find(TOTAL_KEY)? + TOTAL_KEY.len();
    let digits: String =
        marker[t..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
    let total_lines: u32 = digits.parse().ok()?;

    let head = &output[..m];
    let body = match head.find(CONTENT_HEADER) {
        Some(i) => &head[i + CONTENT_HEADER.len()..],
        None => head,
    };
    let body = body.trim_start_matches(['\r', '\n']).trim_end();
    let n = body.lines().count() as u32;
    let partial = body.lines().last().is_some_and(|l| l.trim_end().ends_with("..."));
    let resume_line = if partial { n.max(1) } else { n + 1 };
    if resume_line > total_lines {
        return None;
    }
    Some(Truncation { resume_line, total_lines })
}

/// (start_line, max_lines) pairs covering `resume..=total`.
pub fn plan_chunks(t: &Truncation) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut s = t.resume_line;
    while s <= t.total_lines && out.len() < MAX_CHUNKS {
        let n = CHUNK_LINES.min(t.total_lines - s + 1);
        out.push((s, n));
        s += n;
    }
    out
}

/// Removes the truncation marker (and anything after it) from `output`.
fn strip_marker(output: &str) -> &str {
    match output.find(MARKER_PREFIX) {
        Some(i) => output[..i].trim_end(),
        None => output,
    }
}

/// If `first` is truncated, fetch the rest via `read_chunk(start_line, max_lines)`
/// and return one combined evidence string with the marker removed. Chunk
/// outputs are appended verbatim, so their `[READ_CHUNK_OK] ... Lines a-b of N`
/// headers still feed the existing coverage ledger. Non-truncated input is
/// returned unchanged.
pub fn complete_read<F>(first: &str, mut read_chunk: F) -> Result<String>
where
    F: FnMut(u32, u32) -> Result<String>,
{
    let Some(t) = detect(first) else {
        return Ok(first.to_string());
    };
    let mut combined = strip_marker(first).to_string();
    for (start, n) in plan_chunks(&t) {
        combined.push('\n');
        combined.push_str(read_chunk(start, n)?.trim_end());
    }
    combined.push('\n');
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(partial: bool) -> String {
        let tail = if partial { "      ..." } else { "line 3" };
        format!(
            "[READ_OK] a.rs\nFULL FILE CONTENT:\nline 1\nline 2\n{tail}\n\n[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: 194. USE THE `read_file_chunk` TOOL ...]"
        )
    }

    #[test]
    fn detects_partial_last_line() {
        assert_eq!(detect(&sample(true)), Some(Truncation { resume_line: 3, total_lines: 194 }));
    }

    #[test]
    fn detects_clean_cut() {
        assert_eq!(detect(&sample(false)), Some(Truncation { resume_line: 4, total_lines: 194 }));
    }

    #[test]
    fn untruncated_is_none() {
        assert_eq!(detect("[READ_OK] a.rs\nFULL FILE CONTENT:\nx\n"), None);
    }

    #[test]
    fn chunks_cover_to_end_without_gap() {
        let c = plan_chunks(&Truncation { resume_line: 3, total_lines: 194 });
        assert_eq!(c.first(), Some(&(3, 80)));
        let (s, n) = *c.last().unwrap();
        assert_eq!(s + n - 1, 194);
        for w in c.windows(2) {
            assert_eq!(w[0].0 + w[0].1, w[1].0);
        }
    }

    #[test]
    fn complete_read_strips_marker_and_appends() {
        let out = complete_read(&sample(true), |s, n| {
            Ok(format!(
                "[READ_CHUNK_OK] File: a.rs | Lines {}-{} of 194\nCONTENT:\nx",
                s,
                s + n - 1
            ))
        })
        .unwrap();
        assert!(!out.contains("TRUNCATED"));
        assert!(out.contains("Lines 3-82 of 194"));
        assert!(out.contains("Lines 163-194 of 194"));
    }

    #[test]
    fn passthrough_when_not_truncated() {
        let s = "[READ_OK] a\nFULL FILE CONTENT:\nx";
        assert_eq!(complete_read(s, |_, _| unreachable!()).unwrap(), s);
    }
}
