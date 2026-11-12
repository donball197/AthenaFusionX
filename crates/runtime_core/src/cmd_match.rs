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
    strip_directive_suffix(raw).replace("\\n", " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Directive phrasing often wraps a command as "Run: <command> and report
/// stdout." When the strategy string carries that wrapper, it will not match
/// the bare command recorded in `[CMD_OK]` evidence. Strip the trailing
/// report-style phrase before normalizing so both sides converge.
fn strip_directive_suffix(s: &str) -> &str {
    let s = s.trim_end_matches(|c: char| c == '.' || c == '!' || c == '?');
    for suffix in [
        " and report command and full stdout",
        " and report full stdout",
        " and report command",
        " and report its output",
        " and report the output verbatim",
        " and report the output",
        " and report the result",
        " and report the results",
        " and report the findings",
        " and report findings",
        " and report stdout",
        " 2>&1",
        " 2>/dev/null",
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            return stripped.trim_end();
        }
    }
    s
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
/// Strip a leading `cd <workspace> &&` or `cd <workspace>;` when the
/// target canonicalizes to the current working directory. Applied at both
/// emission (executor) and verification (command_condition_met,
/// cmd_ok_block_matches) so a strategist that prefixes `cd $WORKSPACE &&`
/// still satisfies a contract whose command lacks the `cd`.
///
/// No-op when the target differs from cwd or cannot be canonicalized, so
/// `cd /tmp && ...` is never stripped. Safe because the daemon never chdirs
/// after startup, so cwd is stable between emission and verification.
pub fn strip_workspace_cd_prefix(cmd: &str) -> &str {
    let s = cmd.trim_start();
    let Some(rest) = s.strip_prefix("cd ") else { return s };
    let (target_end, sep_len) = if let Some(p) = rest.find("&&") {
        (p, 2)
    } else if let Some(p) = rest.find(';') {
        (p, 1)
    } else {
        return s;
    };
    let target = rest[..target_end].trim();
    let bare = target.trim_matches(|c| c == '"' || c == '\'');
    let Ok(cwd) = std::env::current_dir() else { return s };
    let Ok(canon) = std::fs::canonicalize(bare) else { return s };
    if cwd != canon {
        return s;
    }
    rest[target_end + sep_len..].trim_start()
}

pub fn command_condition_met(cmd: &str, evidence: &str) -> bool {
    // Reset after mission-level rejection. If [HALT_REJECTED] is more recent
    // than the last [CMD_OK], a halt was already submitted using that evidence
    // and the mission verifier found it insufficient. Require fresh command
    // output before returning true again, so the strategist's pending command
    // actually runs instead of being pre-empted by an identical auto-halt.
    let rejected_after_last_ok =
        match (evidence.rfind("[HALT_REJECTED]"), evidence.rfind("[CMD_OK]")) {
            (Some(r), Some(o)) => r > o,
            (Some(_), None) => true,
            _ => false,
        };
    if rejected_after_last_ok {
        return false;
    }

    let stripped = strip_workspace_cd_prefix(cmd);
    let want = cmd_hash(stripped);
    if hash_marker_present(&want, evidence) || cmd_ok_block_matches(&want, evidence) {
        return true;
    }
    // Segment fallback: a strategist may propose a compound command
    // (e.g. `git branch && git rev-parse && git status --short`) that
    // contains the required command as one segment. The required command
    // did run; it just wasn't the sole content of the block. Match on any
    // `&&`/`;`/`|` segment of any recorded [CMD_OK] Command payload.
    command_segment_met(stripped, evidence)
}

/// True when any segment of any successful `[CMD_OK] Command:` payload
/// matches the required command after normalization.
fn command_segment_met(want_cmd: &str, evidence: &str) -> bool {
    let want = normalize_cmd(want_cmd);
    if want.is_empty() {
        return false;
    }

    let mut capture: Option<String> = None;

    for line in evidence.lines() {
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("[CMD_OK] Command: ") {
            // A new command record supersedes any incomplete record.
            capture = Some(rest.to_string());
            continue;
        }

        if let Some(buf) = capture.as_mut() {
            if trimmed.starts_with("Status:") {
                // Segment matching is authorized only by a clean exit.
                if status_line_ok(trimmed)
                    && buf.lines().any(|command_line| segment_match_line(command_line, &want))
                {
                    return true;
                }

                capture = None;
            } else if trimmed.starts_with('[') && trimmed.contains(']') {
                // Malformed or interrupted command record.
                capture = None;
            } else {
                buf.push('\n');
                buf.push_str(line);
            }
        }
    }

    false
}

fn segment_match_line(line: &str, want_norm: &str) -> bool {
    for seg in line.split(|c| matches!(c, '&' | ';' | '|')) {
        let s = seg.trim();
        if s.is_empty() {
            continue;
        }
        if normalize_cmd(s) == want_norm {
            return true;
        }
    }
    false
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
                if status_line_ok(trimmed) && cmd_hash(strip_workspace_cd_prefix(buf)) == want {
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

    #[cfg(test)]
    mod strip_directive_suffix_tests {
        use super::{normalize_cmd, strip_directive_suffix};

        #[test]
        fn strips_and_report_stdout() {
            assert_eq!(
                strip_directive_suffix("find X | wc -l and report stdout."),
                "find X | wc -l"
            );
        }

        #[test]
        fn strips_and_report_command_and_full_stdout() {
            assert_eq!(
                strip_directive_suffix("grep -rn epoch vendor/ and report command and full stdout"),
                "grep -rn epoch vendor/"
            );
        }

        #[test]
        fn leaves_bare_command_alone() {
            assert_eq!(strip_directive_suffix("find X | wc -l"), "find X | wc -l");
            assert_eq!(strip_directive_suffix("grep -rn wasm vendor/"), "grep -rn wasm vendor/");
        }

        #[test]
        fn normalize_makes_suffixed_and_bare_identical() {
            assert_eq!(
                normalize_cmd("find X | wc -l and report stdout."),
                normalize_cmd("find X | wc -l")
            );
        }

        #[test]
        fn strips_stderr_redirects() {
            assert_eq!(
                strip_directive_suffix("cargo check --workspace 2>&1"),
                "cargo check --workspace"
            );
            assert_eq!(
                strip_directive_suffix("cargo check --workspace 2>/dev/null"),
                "cargo check --workspace"
            );
        }

        #[test]
        fn normalize_stderr_redirect_matches_bare() {
            assert_eq!(
                normalize_cmd("cargo check --workspace 2>&1"),
                normalize_cmd("cargo check --workspace")
            );
        }
    }
}

#[cfg(test)]
mod literal_newline_tests {
    use super::cmd_hash;

    #[test]
    fn literal_backslash_n_equals_real_newline() {
        assert_eq!(cmd_hash("printf '%s\\n' 'A' && pwd"), cmd_hash("printf '%s\n' 'A' && pwd"));
    }
}
