//! Shared evidence-path matching and marker classification.
//!
//! Single source of truth for semantics previously duplicated across
//! orchestrator_v2/verifier.rs (read_evidence_matches_path),
//! outcome_verifier.rs (ReadPathAcquired), and cortical_loop.rs's
//! confirmed-absence check.

/// Lexically normalize a slash-separated path. Resolves `.` and `..` and
/// collapses duplicate slashes. Preserves a leading `/` so absolute and
/// relative spellings remain distinguishable. `fs::canonicalize` is
/// deliberately not used: confirmed-absence targets do not exist on disk.
pub fn normalize_path(p: &str) -> String {
    // Expand `~` and `~/...` to $HOME before further normalization so a
    // contract term like `~/AthenaFusionX` compares equal to evidence
    // recorded as `/root/AthenaFusionX`. Only the exact forms `~` and
    // `~/...` expand; `~user/...` and mid-path `~` are left unchanged.
    let trimmed = p.trim();
    let expanded: String;
    let t: &str = if trimmed == "~" || trimmed.starts_with("~/") {
        match std::env::var("HOME") {
            Ok(home) if !home.is_empty() => {
                expanded = trimmed.replacen("~", &home, 1);
                expanded.as_str()
            }
            _ => trimmed,
        }
    } else {
        trimmed
    };
    let mut out: Vec<&str> = Vec::new();
    for seg in t.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if t.starts_with('/') {
        format!("/{}", joined)
    } else {
        joined
    }
}

/// True when both spellings denote the same location. Only the
/// absolute-vs-workspace-relative mismatch is bridged:
///
///   /root/x/vendor/a.rs == vendor/a.rs
///   vendor/a.rs != a.rs (arbitrary suffix overlap does not match)
pub fn paths_equivalent(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize_path(a), normalize_path(b));
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    match (na.starts_with('/'), nb.starts_with('/')) {
        (true, false) => na.ends_with(&format!("/{}", nb)),
        (false, true) => nb.ends_with(&format!("/{}", na)),
        _ => false,
    }
}

/// True only for missing-target errors. Sandbox denials (PATH_TRAVERSAL_DENIED,
/// permission errors) inspected nothing and must never satisfy an inspection
/// criterion. Unrecognized kinds fail closed.
pub fn is_confirmed_absence(failure_tail: &str) -> bool {
    let err = failure_tail
        .split("OS rejected action")
        .last()
        .unwrap_or(failure_tail)
        .to_ascii_lowercase();
    if err.contains("path_traversal_denied")
        || err.contains("permission")
        || err.contains("denied")
        || err.contains("forbidden")
    {
        return false;
    }
    err.contains("enoent") || err.contains("no such file") || err.contains("entity not found") || err.contains("does not exist") || err.contains("path_not_found")
}

/// True for an actual uppercase evidence marker such as `[READ_OK]`,
/// `[CMD_OK] Command: ..`, `[DAG NODE COMPLETE]`. Bracketed source-code
/// content such as `[1, 2, 3]` returns false, so a bounded scan over command
/// output does not terminate early on source code that merely starts with `[`.
pub fn is_marker_line(line: &str) -> bool {
    let t = line.trim_start();
    let Some(rest) = t.strip_prefix('[') else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    let token = &rest[..end];
    !token.is_empty() && token.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c == ' ')
}

/// Shared CMD_OK binder. Authorizes only a real execution record: a
/// line-anchored (trimmed) `[CMD_OK]` whose record carries
/// `Status: exit status: 0`, either inline or on a following line bounded
/// by the next marker line.
pub fn cmd_ok_binds_command(evidence: &str) -> bool {
    let lines: Vec<&str> = evidence.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("[CMD_OK]") else {
            continue;
        };
        if let Some(status) = rest.trim_start().strip_prefix("Status:") {
            if status.trim() == "exit status: 0" {
                return true;
            }
        }
        if lines[i + 1..]
            .iter()
            .take_while(|l| !is_marker_line(l))
            .any(|l| l.trim() == "Status: exit status: 0")
        {
            return true;
        }
    }
    false
}

/// Four-shape read-evidence matcher. Accepted evidence:
///   * [READ_OK] <path>
///   * [READ_CHUNK_OK] File: <path> | ...
///   * confirmed-absence [ACTION FAILED] path=<path> OS rejected action: ENOENT...
///   * [SEARCH_CODE_OK] Path: <path> | ...
#[allow(dead_code)]
pub fn read_path_satisfied(evidence: &str, expected: &str) -> bool {
    evidence.lines().any(|line| {
        if let Some(rest) = line.strip_prefix("[READ_OK]") {
            return paths_equivalent(rest, expected);
        }
        if let Some(rest) = line.strip_prefix("[READ_CHUNK_OK] File: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| paths_equivalent(p, expected))
                .unwrap_or(false);
        }
        if let Some(rest) = line.strip_prefix("[ACTION FAILED] path=") {
            let path_part = rest.split(" OS rejected action").next().unwrap_or("");
            return paths_equivalent(path_part, expected) && is_confirmed_absence(rest);
        }
        if let Some(rest) = line.strip_prefix("[SEARCH_CODE_OK] Path: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| paths_equivalent(p, expected))
                .unwrap_or(false);
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absence_filter_rejects_sandbox_denials() {
        assert!(is_confirmed_absence(
            "path=vendor/new.rs OS rejected action: ENOENT: No such file or directory"
        ));
        assert!(!is_confirmed_absence(
            "path=/tmp/x.py OS rejected action: PATH_TRAVERSAL_DENIED: outside workspace"
        ));
        assert!(!is_confirmed_absence("OS rejected action: EACCES: permission denied"));
    }

    #[test]
    fn marker_lines_exclude_bracketed_code() {
        assert!(is_marker_line("[READ_OK] crates/x.rs"));
        assert!(is_marker_line("  [CMD_OK] Command: ls"));
        assert!(is_marker_line("[DAG NODE COMPLETE]"));
        assert!(!is_marker_line("[1, 2, 3]"));
        assert!(!is_marker_line("[\"ACTION\", \"x\"]"));
    }

    #[test]
    fn equivalence_bridges_only_abs_vs_relative() {
        assert!(paths_equivalent(
            "/root/AthenaFusionX/vendor/external_repo/Cargo.toml",
            "vendor/external_repo/Cargo.toml"
        ));
        assert!(paths_equivalent("./vendor/a.rs", "vendor/a.rs"));
        assert!(paths_equivalent("/root/x/a.rs", "/root/x/a.rs"));
        assert!(!paths_equivalent("vendor/a.rs", "a.rs"));
        assert!(!paths_equivalent("a.rs", "vendor/a.rs"));
        assert!(!paths_equivalent("vendor/a.rs", "vendor/b.rs"));
    }

    #[test]
    fn read_matcher_accepts_confirmed_absence_only() {
        let enoent = "[ACTION FAILED] path=vendor/new.rs OS rejected action: ENOENT: No such file";
        assert!(read_path_satisfied(enoent, "vendor/new.rs"));
        let denied = "[ACTION FAILED] path=/tmp/x.py OS rejected action: PATH_TRAVERSAL_DENIED: outside workspace";
        assert!(!read_path_satisfied(denied, "/tmp/x.py"));
    }

    #[test]
    fn cmd_ok_binder_handles_long_short_spoof_and_leak() {
        let long = "[CMD_OK] Command: python3 -c 'x'\n[1, 2, 3]\nStatus: exit status: 0\n";
        assert!(cmd_ok_binds_command(long));
        assert!(cmd_ok_binds_command("[CMD_OK] Status: exit status: 0\n"));
        assert!(!cmd_ok_binds_command("[READ_OK] notes.md\n[CMD_OK]\n"));
        assert!(!cmd_ok_binds_command(
            "[CMD_OK] Command: ls\n[DAG NODE COMPLETE]\nStatus: exit status: 0\n"
        ));
    }
}


#[cfg(test)]
mod tilde_tests {
    use super::{normalize_path, paths_equivalent};

    #[test]
    fn tilde_expands_to_home() {
        let home = std::env::var("HOME").expect("HOME must be set");
        assert_eq!(normalize_path("~"), normalize_path(&home));
    }

    #[test]
    fn tilde_prefix_expands_to_home_prefix() {
        let home = std::env::var("HOME").expect("HOME must be set");
        assert_eq!(
            normalize_path("~/AthenaFusionX"),
            normalize_path(&format!("{}/AthenaFusionX", home))
        );
    }

    #[test]
    fn tilde_contract_matches_absolute_evidence() {
        let home = std::env::var("HOME").expect("HOME must be set");
        assert!(paths_equivalent(
            "~/AthenaFusionX",
            &format!("{}/AthenaFusionX", home)
        ));
    }

    #[test]
    fn tilde_user_is_not_expanded() {
        assert_eq!(normalize_path("~user/x"), "~user/x");
    }

    #[test]
    fn mid_path_tilde_is_not_expanded() {
        assert_eq!(normalize_path("/foo/~/bar"), "/foo/~/bar");
    }
}
