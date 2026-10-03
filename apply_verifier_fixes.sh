#!/usr/bin/env bash
# apply_verifier_fixes.sh — v3
#
# Surgical verifier-layer fixes. All edits anchor-exact with
# single-occurrence enforcement and all-or-nothing application.
#
# Fixes applied:
#   * new module   crates/runtime_core/src/evidence_paths.rs
#   * lib.rs       `pub mod evidence_paths;` inserted if absent
#   * OV           fail-closed empty contract, telemetry stub deleted,
#                  ReadPathAcquired/READ_CHUNK/SEARCH_CODE/absence via
#                  paths_equivalent + is_confirmed_absence,
#                  marker arms line-anchored, CMD_OK via shared binder
#   * VF           same read-path/absence/marker migrations, CMD_OK scan
#                  bounded by is_marker_line
#
# Usage:
#   ./apply_verifier_fixes.sh --dry-run
#   ./apply_verifier_fixes.sh
#   RUN_TESTS=1 ./apply_verifier_fixes.sh
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== runtime_core verifier fixes v3 | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

resolve() {
    local rel="$1" abs
    abs="$(realpath -m "$WORKSPACE_ROOT/$rel")"
    case "$abs" in
        "$WORKSPACE_ROOT"/*) printf '%s' "$abs" ;;
        *) echo "[PATH_TRAVERSAL_DENIED] $rel outside workspace" >&2; exit 1 ;;
    esac
}

OV="crates/runtime_core/src/outcome_verifier.rs"
VF="crates/runtime_core/src/orchestrator_v2/verifier.rs"
LIB="crates/runtime_core/src/lib.rs"
EP="crates/runtime_core/src/evidence_paths.rs"

for rel in "$OV" "$VF" "$LIB"; do
    [[ -f "$WORKSPACE_ROOT/$rel" ]] || { echo "[PREFLIGHT FAILED] missing $rel" >&2; exit 1; }
done

# ---------------------------------------------------------------------------
# NEW MODULE
# ---------------------------------------------------------------------------
NEW_MODULE="$(cat <<'EOF'
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
    let t = p.trim();
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
    err.contains("enoent")
        || err.contains("no such file")
        || err.contains("entity not found")
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
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c == ' ')
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
            "/root/AthenaFusionX/vendor/openfang/Cargo.toml",
            "vendor/openfang/Cargo.toml"
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
EOF
)"

# ---------------------------------------------------------------------------
# WRITE / VERIFY NEW MODULE (never silently overwrite different content)
# ---------------------------------------------------------------------------
if [[ -f "$WORKSPACE_ROOT/$EP" ]]; then
    if [[ "$(cat "$WORKSPACE_ROOT/$EP")" == "$NEW_MODULE" ]]; then
        echo "[UNCHANGED] $EP (byte-identical)"
    else
        echo "[REFUSING] $EP exists with different content; move it aside and re-run." >&2
        exit 1
    fi
elif $DRY_RUN; then
    echo "[DRY-RUN] would create $EP"
else
    mkdir -p "$(dirname "$WORKSPACE_ROOT/$EP")"
    printf '%s\n' "$NEW_MODULE" > "$WORKSPACE_ROOT/$EP"
    echo "[WROTE] $EP"
fi

# ---------------------------------------------------------------------------
# ANCHOR PATCHES (all-or-nothing; single-occurrence per anchor)
# ---------------------------------------------------------------------------
NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
OV_REL="$OV" VF_REL="$VF" LIB_REL="$LIB" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
OV   = os.environ["OV_REL"]
VF   = os.environ["VF_REL"]
LIB  = os.environ["LIB_REL"]

fixes = [
# ── outcome_verifier.rs ────────────────────────────────────────────────────
(OV, "V1 fail-closed empty contract + drop telemetry override",
"""        // 1. If telemetry is provided, it overrides all else (Phase 2 external proof)
        if let Some(telemetry_json) = extract_telemetry(local_evidence) {
            if telemetry_json.exit_code == 0 && telemetry_json.test_digest.is_some() {
                return Ok(());
            } else {
                return Err(format!(
                    "Telemetry captured, but verification failed. Exit code: {}, Tests passed: {}",
                    telemetry_json.exit_code,
                    telemetry_json.test_digest.is_some()
                ));
            }
        }""",
"""        // FAIL CLOSED: an empty contract authorizes nothing. Universal
        // quantification over zero conditions is vacuously true and would let
        // an unrecognized mission authorize itself.
        if conditions.is_empty() {
            return Err("empty completion contract is never satisfied".to_string());
        }

        // Former "telemetry overrides all else" branch removed. External
        // proof, when implemented, must be an ADDITIONAL contract condition
        // rather than a substitution for the declared conditions."""),

(OV, "V1b1 rename empty-contract test",
"""    fn empty_condition_fallback_requires_a_write_marker() {""",
"""    fn empty_contract_fails_closed() {"""),

(OV, "V1b2 flip write-marker assertion",
"""        assert!(OutcomeVerifier::mission_completion_authorized("[WRITE_OK] out.md", 1.0, &[]));""",
"""        assert!(!OutcomeVerifier::mission_completion_authorized("[WRITE_OK] out.md", 1.0, &[]));"""),

(OV, "V1c delete dead telemetry stub",
"""// Helper to extract JSON artifact appended by the RoboticExecutor
fn extract_telemetry(_local_evidence: &str) -> Option<TelemetryArtifact> {
    // Assumed to exist in the original file or cognitive_events
    // If missing, returns None and skips telemetry check
    None
}""",
"""// extract_telemetry removed: it was permanently stubbed to None behind a
// branch that "overrode all else" -- a dormant bypass. External proof must
// return as a CompletionCondition variant, never as a substitution."""),

(OV, "V2 READ_OK -> paths_equivalent",
"""                        if let Some(rest) = l.strip_prefix("[READ_OK]") {
                            rest.trim() == expected""",
"""                        if let Some(rest) = l.strip_prefix("[READ_OK]") {
                            crate::evidence_paths::paths_equivalent(rest.trim(), expected)"""),

(OV, "V2b READ_CHUNK -> paths_equivalent",
"""                        } else if let Some(rest) = l.strip_prefix("[READ_CHUNK_OK] File: ") {
                            rest.split(" | ").next().map(|p| p.trim() == expected).unwrap_or(false)""",
"""                        } else if let Some(rest) = l.strip_prefix("[READ_CHUNK_OK] File: ") {
                            rest
                                .split(" | ")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)"""),

(OV, "V2c SEARCH_CODE -> paths_equivalent",
"""                            // decayed to 0.00 across 11 turns).
                            rest.split(" | ").next().map(|p| p.trim() == expected).unwrap_or(false)""",
"""                            // decayed to 0.00 across 11 turns). Path
                            // comparison uses the shared paths_equivalent.
                            rest
                                .split(" | ")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)"""),

(OV, "V3 absence -> confirmed-absence only",
"""                        } else if let Some(rest) = l.strip_prefix("[ACTION FAILED] path=") {
                            // A path-qualified read failure (e.g. ENOENT) is
                            // still a completed, evidenced inspection of the
                            // target location -- the location was checked
                            // and found empty. See cortical_loop.rs's
                            // motor-command error formatting for where
                            // "path=<p> " is emitted.
                            rest.split(" OS rejected action")
                                .next()
                                .map(|p| p.trim() == expected)
                                .unwrap_or(false)""",
"""                        } else if let Some(rest) = l.strip_prefix("[ACTION FAILED] path=") {
                            // CONFIRMED-ABSENCE ONLY. ENOENT proves the target
                            // was inspected and found absent; sandbox denials
                            // inspected nothing. See
                            // evidence_paths::is_confirmed_absence.
                            rest.split(" OS rejected action")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)
                                && crate::evidence_paths::is_confirmed_absence(rest)"""),

(OV, "V4 ReadMarkerAcquired -> line-anchored",
"""                CompletionCondition::ReadMarkerAcquired => {
                    local_evidence.contains("[READ_OK]")
                        || local_evidence.contains("[READ_CHUNK_OK]")
                }""",
"""                CompletionCondition::ReadMarkerAcquired => {
                    local_evidence.lines().any(|l| {
                        let t = l.trim_start();
                        t.starts_with("[READ_OK]") || t.starts_with("[READ_CHUNK_OK]")
                    })
                }"""),

(OV, "V5 SearchMarkerAcquired -> line-anchored",
"""                CompletionCondition::SearchMarkerAcquired => {
                    local_evidence.contains("[SEARCH_CODE_OK]")
                }""",
"""                CompletionCondition::SearchMarkerAcquired => {
                    local_evidence
                        .lines()
                        .any(|l| l.trim_start().starts_with("[SEARCH_CODE_OK]"))
                }"""),

(OV, "V6 SearchMemoryMarkerAcquired -> line-anchored",
"""                CompletionCondition::SearchMemoryMarkerAcquired => {
                    local_evidence.contains("[SEARCH_MEMORY_OK]")
                }""",
"""                CompletionCondition::SearchMemoryMarkerAcquired => {
                    local_evidence
                        .lines()
                        .any(|l| l.trim_start().starts_with("[SEARCH_MEMORY_OK]"))
                }"""),

(OV, "V7 WriteMarkerAcquired -> line-anchored",
"""                CompletionCondition::WriteMarkerAcquired => local_evidence.contains("[WRITE_OK]"),""",
"""                CompletionCondition::WriteMarkerAcquired => {
                    local_evidence
                        .lines()
                        .any(|l| l.trim_start().starts_with("[WRITE_OK]"))
                }"""),

(OV, "V8 CommandMarkerAcquired -> shared binder",
"""                    let lines: Vec<&str> = local_evidence.lines().collect();
                    let mut authorized = false;
                    for (i, line) in lines.iter().enumerate() {
                        let t = line.trim_start();
                        let Some(rest) = t.strip_prefix("[CMD_OK]") else {
                            continue;
                        };
                        let inline = rest
                            .strip_prefix(" Status:")
                            .or_else(|| rest.strip_prefix("Status:"));
                        let mut ok = inline
                            .map(|s| s.trim().ends_with("exit status: 0"))
                            .unwrap_or(false);
                        if !ok {
                            if let Some(next) = lines.get(i + 1) {
                                if let Some(s) = next.trim().strip_prefix("Status:") {
                                    ok = s.trim().ends_with("exit status: 0");
                                }
                            }
                        }
                        if ok {
                            authorized = true;
                            break;
                        }
                    }
                    authorized""",
"""                    // Delegates to the shared binder in evidence_paths. The
                    // former 1-line-forward scan mishandled multi-line
                    // commands and bracketed output.
                    crate::evidence_paths::cmd_ok_binds_command(local_evidence)"""),

# ── orchestrator_v2/verifier.rs ────────────────────────────────────────────
(VF, "R1 READ_OK -> paths_equivalent",
"""        if let Some(path) = line.strip_prefix("[READ_OK]") {
            return path.trim() == expected_path;""",
"""        if let Some(path) = line.strip_prefix("[READ_OK]") {
            return crate::evidence_paths::paths_equivalent(path.trim(), expected_path);"""),

(VF, "R2 READ_CHUNK -> paths_equivalent",
"""        if let Some(rest) = line.strip_prefix("[READ_CHUNK_OK] File: ") {
            return rest.split(" | ").next().map(str::trim) == Some(expected_path);""",
"""        if let Some(rest) = line.strip_prefix("[READ_CHUNK_OK] File: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false);"""),

(VF, "R3 absence -> confirmed-absence only",
"""        if let Some(rest) = line.strip_prefix("[ACTION FAILED] path=") {
            return rest.split(" OS rejected action").next().map(str::trim) == Some(expected_path);""",
"""        if let Some(rest) = line.strip_prefix("[ACTION FAILED] path=") {
            // CONFIRMED-ABSENCE ONLY (see evidence_paths::is_confirmed_absence).
            return rest
                .split(" OS rejected action")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false)
                && crate::evidence_paths::is_confirmed_absence(rest);"""),

(VF, "R4 SEARCH_CODE -> paths_equivalent",
"""        if let Some(rest) = line.strip_prefix("[SEARCH_CODE_OK] Path: ") {
            return rest.split(" | ").next().map(str::trim) == Some(expected_path);""",
"""        if let Some(rest) = line.strip_prefix("[SEARCH_CODE_OK] Path: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false);"""),

(VF, "R5a WRITE_OK arm line-anchored",
"""            } else if let Some(path) = criterion.strip_prefix("WRITE_OK:") {
                evidence.contains("[WRITE_OK]") && evidence.contains(path)""",
"""            } else if let Some(path) = criterion.strip_prefix("WRITE_OK:") {
                let target = path.trim();
                let synthesized =
                    format!("Successfully synthesized and wrote report to {}", target);
                evidence.lines().any(|l| {
                    l.strip_prefix("[WRITE_OK]")
                        .map(|rest| {
                            let rest = rest.trim();
                            rest == target || rest == synthesized
                        })
                        .unwrap_or(false)
                })"""),

(VF, "R5b SEARCH_MEMORY arm line-anchored",
"""            } else if criterion == "SEARCH_MEMORY_OK" {
                evidence.contains("[SEARCH_MEMORY_OK]")""",
"""            } else if criterion == "SEARCH_MEMORY_OK" {
                evidence
                    .lines()
                    .any(|l| l.trim_start().starts_with("[SEARCH_MEMORY_OK]"))"""),

(VF, "R6 CMD_OK scan -> trimmed + is_marker_line bound",
"""                let lines: Vec<&str> = evidence.lines().collect();
                lines.iter().enumerate().any(|(i, line)| {
                    line.starts_with("[CMD_OK]")
                        && lines[i + 1..]
                            .iter()
                            .take_while(|l| !l.starts_with('['))
                            .any(|l| l.trim() == "Status: exit status: 0")
                })""",
"""                let lines: Vec<&str> = evidence.lines().collect();
                lines.iter().enumerate().any(|(i, line)| {
                    // Trimmed: an indented [CMD_OK] counts. Scan bounded by
                    // is_marker_line so bracketed command output does not
                    // truncate the scan early.
                    line.trim_start().starts_with("[CMD_OK]")
                        && lines[i + 1..]
                            .iter()
                            .take_while(|l| !crate::evidence_paths::is_marker_line(l))
                            .any(|l| l.trim() == "Status: exit status: 0")
                })"""),
]

# ── Read all sources before any write. ─────────────────────────────────────
texts = {rel: (root / rel).read_text() for rel in (OV, VF, LIB)}

# ── lib.rs module declaration. ─────────────────────────────────────────────
lib_text = texts[LIB]
if "pub mod evidence_paths;" in lib_text:
    new_lib_text = lib_text
    print("[PREFLIGHT] lib.rs already exports evidence_paths")
else:
    anchor = "pub mod evidence;"
    if lib_text.count(anchor) != 1:
        print("[PREFLIGHT FAILED] lib.rs: 'pub mod evidence;' not found exactly once")
        sys.exit(1)
    new_lib_text = lib_text.replace(
        anchor, "pub mod evidence;\npub mod evidence_paths;", 1
    )
    print("[PREFLIGHT] lib.rs: will insert pub mod evidence_paths;")

# ── Anchor preflight. ──────────────────────────────────────────────────────
problems = []
for rel, name, anchor, _ in fixes:
    n = texts[rel].count(anchor)
    if n != 1:
        problems.append(f"ANCHOR x{n} (want 1): {name} @ {rel}")
if problems:
    print("[PREFLIGHT FAILED] no files modified:")
    for p in problems:
        print("  -", p)
    sys.exit(1)
print(f"[PREFLIGHT OK] {len(fixes)} anchors matched once")

# ── Apply in memory + diff. ────────────────────────────────────────────────
dirty = {OV: texts[OV], VF: texts[VF], LIB: new_lib_text}
for rel, name, anchor, repl in fixes:
    dirty[rel] = dirty[rel].replace(anchor, repl, 1)

for rel in (LIB, OV, VF):
    diff = list(difflib.unified_diff(
        texts[rel].splitlines(keepends=True),
        dirty[rel].splitlines(keepends=True),
        fromfile=f"a/{rel}", tofile=f"b/{rel}",
    ))
    print(f"\n--- {rel} ---")
    sys.stdout.writelines(diff)

if dry:
    print("\n[DRY-RUN] no files written.")
else:
    for rel in (LIB, OV, VF):
        (root / rel).write_text(dirty[rel])
        print(f"[PATCH_OK] {rel}")
PYEOF

# ---------------------------------------------------------------------------
# Post-apply information greps (never fail the run).
# ---------------------------------------------------------------------------
if ! $DRY_RUN; then
    echo
    echo "=== [VERIFY] residual references ==="
    grep -RIn 'extract_telemetry\|TelemetryArtifact' \
        crates/runtime_core/src --include='*.rs' || echo "  (none)"
    echo "--- bare-suffix scan bounds remaining ---"
    grep -RIn "take_while(|l| !l.starts_with" \
        crates/runtime_core/src --include='*.rs' || echo "  (none)"
    echo "--- empty-contract call sites ---"
    grep -RIn 'mission_completion_authorized([^)]*&\[\])' \
        crates/runtime_core/src --include='*.rs' || echo "  (none)"
    echo "--- bare path equality remnants in the two verifiers ---"
    grep -RIn 'p\.trim() == expected\|== Some(expected_path)' \
        crates/runtime_core/src/outcome_verifier.rs \
        crates/runtime_core/src/orchestrator_v2/verifier.rs || echo "  (none)"
fi

# ---------------------------------------------------------------------------
if $DRY_RUN; then
    echo
    echo "[DRY-RUN] complete. Review the diffs above, then re-run without --dry-run."
else
    echo
    echo "[OK] verifier fixes applied under $WORKSPACE_ROOT/crates/runtime_core/src/"
    if [[ "${RUN_TESTS:-0}" == "1" ]]; then
        echo ">> Running runtime_core tests..."
        (cd "$WORKSPACE_ROOT" && cargo test -p runtime_core)
    else
        echo ">> Tests not run. Use: RUN_TESTS=1 $0"
    fi
    echo ">> Review with: git diff --stat && git diff crates/runtime_core/src"
fi
