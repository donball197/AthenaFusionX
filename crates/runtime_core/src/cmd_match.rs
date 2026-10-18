//! Normalized command identity for `CommandExecutionVerified`.
//!
//! The historical bug: the condition matched the raw command string while
//! anti-loop blocked re-executing that same string -> unsatisfiable after the
//! first [CMD_OK]. Identity is now content-based (normalized + hashed), and
//! the matcher accepts either a [CMD_HASH] marker or a legacy [CMD_OK] block.
//!
//! Success proof is mandatory on both paths:
//!   * [CMD_HASH] is emitted only on a clean exit (gated at the executor),
//!   * a legacy [CMD_OK] block credits only when its status line shows
//!     exit status 0.
#![allow(dead_code)]

use sha2::{Digest, Sha256};

/// Collapse all whitespace runs to a single space and trim.
/// Both sides of every comparison pass through this, so interior whitespace
/// differences (JSON \n renderings vs. live newlines) no longer create
/// distinct identities.
pub fn normalize_cmd(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn cmd_hash(raw: &str) -> String {
    format!("{:x}", Sha256::digest(normalize_cmd(raw).as_bytes()))
}

/// Call at MotorCortex emit time, right where [CMD_OK] is written.
pub fn cmd_hash_marker(raw: &str) -> String {
    format!("[CMD_HASH] {}", cmd_hash(raw))
}

/// True when the evidence contains [CMD_HASH] <want> or a legacy
/// `[CMD_OK] Command: ... / Status: ...` block whose command hashes to `want`
/// AND whose status line shows a clean exit.
pub fn command_condition_met(cmd: &str, evidence: &str) -> bool {
    let want = cmd_hash(cmd);
    hash_marker_present(&want, evidence) || cmd_ok_block_matches(&want, evidence)
}

fn hash_marker_present(want: &str, evidence: &str) -> bool {
    evidence.lines().any(|l| l.trim().strip_prefix("[CMD_HASH] ").is_some_and(|h| h.trim() == want))
}

/// Legacy path: `[CMD_OK] Command: <cmd...>` may span multiple lines (as with
/// the multi-line `python3 -c` commands in the logs) and ends at a line
/// beginning with `Status:`.
fn cmd_ok_block_matches(want: &str, evidence: &str) -> bool {
    let mut capture: Option<String> = None;
    for line in evidence.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("[CMD_OK] Command: ") {
            capture = Some(rest.to_string());
            continue;
        }
        if let Some(buf) = capture.as_mut() {
            if trimmed.starts_with("Status:") {
                // A command counts as executed ONLY on a clean exit. A
                // non-zero status must never satisfy the condition.
                if status_line_ok(trimmed) && cmd_hash(buf) == want {
                    return true;
                }
                capture = None;
            } else if trimmed.starts_with('[') && trimmed.contains(']') {
                // Next evidence marker arrived before any Status: line: the
                // block is malformed. Do not credit it.
                capture = None;
            } else {
                buf.push('\n');
                buf.push_str(line);
            }
        }
    }
    false // unterminated block: no status proof, no credit
}

/// The [CMD_OK] block's status line as written by the executor
/// ("Status: <ExitStatus>" -> "Status: exit status: 0") and by cortical_loop's
/// physical-evidence path ("Status: exit status: 0").
fn status_line_ok(status: &str) -> bool {
    let s = status.trim();
    s.ends_with("exit status: 0") || s == "Status: 0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_renderings_are_one_identity() {
        let multi = "python3 -c '\nstates = [\"SC\", \"SP\"]\nprint(states)\n'";
        let single = "python3 -c ' states = [\"SC\", \"SP\"] print(states) '";
        assert_eq!(cmd_hash(multi), cmd_hash(single));
    }

    #[test]
    fn hash_marker_roundtrip() {
        let cmd = "python3 -c 'print(1)'";
        let marker = cmd_hash_marker(cmd);
        assert!(command_condition_met(cmd, &marker));
    }

    /// The deadlock scenario: the [CMD_OK] block is in evidence from an
    /// earlier execution; the condition must now be satisfied without
    /// re-running the command.
    #[test]
    fn legacy_cmd_ok_block_satisfies_condition() {
        let cmd = "python3 -c 'print(\"ALL_COMMUTATIVE True\")'";
        let evidence = format!(
            "[CMD_OK] Command: {cmd}\nStatus: exit status: 0\nSTDOUT:\nALL_COMMUTATIVE True\n"
        );
        assert!(command_condition_met(cmd, &evidence));
    }

    #[test]
    fn different_command_does_not_match() {
        let evidence = "[CMD_HASH] 0000";
        assert!(!command_condition_met("ls -la", evidence));
    }

    #[test]
    fn failed_status_block_does_not_satisfy() {
        let cmd = "python3 -c 'print(1)'";
        let evidence = format!("[CMD_OK] Command: {cmd}\nStatus: exit status: 1\n");
        assert!(!command_condition_met(cmd, &evidence));
    }

    #[test]
    fn unterminated_block_is_not_credited() {
        let evidence = "[CMD_OK] Command: ls -la\n";
        assert!(!command_condition_met("ls -la", evidence));
    }
}
