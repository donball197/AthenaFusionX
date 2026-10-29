//! Evidence support for halt evaluation.

const SUCCESS_MARKERS: &[&str] = &[
    "[WRITE_OK]",
    "[CMD_OK]",
    "[READ_OK]",
    "[REPLACE_LINES_OK]",
    "[PATCH_FUNCTION_OK]",
    "[COMPILE_OK]",
];

/// The most recent `max_lines` distinct lines of `evidence` that start with a
/// success marker, oldest first, each cut to 200 characters. Failure markers,
/// vetoes and prose are never included.
pub(crate) fn success_marker_lines(evidence: &str, max_lines: usize) -> String {
    let mut picked: Vec<String> = Vec::new();
    for line in evidence.lines().rev() {
        if picked.len() >= max_lines {
            break;
        }
        let t = line.trim();
        if SUCCESS_MARKERS.iter().any(|m| t.starts_with(m)) {
            let cut: String = t.chars().take(200).collect();
            if !picked.contains(&cut) {
                picked.push(cut);
            }
        }
    }
    picked.reverse();
    picked.join("\n")
}

#[cfg(test)]
mod tests {
    use super::success_marker_lines;

    #[test]
    fn picks_success_markers_and_ignores_everything_else() {
        let ev = "[WRITE_OK] report.py\n[HALT] Deterministic contract is satisfied.\n\
                  [COMPILE_FAIL] boom\n[CRITIC_VETO] no\n[CMD_OK] Command: ls\n";
        assert_eq!(success_marker_lines(ev, 6), "[WRITE_OK] report.py\n[CMD_OK] Command: ls");
    }

    #[test]
    fn keeps_only_the_most_recent_lines_oldest_first() {
        let ev = "[READ_OK] a\n[READ_OK] b\n[READ_OK] c\n";
        assert_eq!(success_marker_lines(ev, 2), "[READ_OK] b\n[READ_OK] c");
    }

    #[test]
    fn duplicate_lines_are_collapsed() {
        let ev = "[WRITE_OK] x.rs\n[WRITE_OK] x.rs\n[WRITE_OK] x.rs\n";
        assert_eq!(success_marker_lines(ev, 6), "[WRITE_OK] x.rs");
    }

    #[test]
    fn empty_when_there_are_no_markers() {
        assert_eq!(success_marker_lines("[HALT] done\nprose only\n", 6), "");
        assert_eq!(success_marker_lines("", 6), "");
    }

    #[test]
    fn long_lines_are_truncated() {
        let ev = format!("[CMD_OK] Command: {}\n", "x".repeat(500));
        assert_eq!(success_marker_lines(&ev, 6).chars().count(), 200);
    }
}
