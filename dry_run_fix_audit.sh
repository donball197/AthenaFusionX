#!/usr/bin/env bash
# dry_run_fix_audit.sh — READ-ONLY verification of the planned audit fixes.
# Writes no files. Exit 0 = all preconditions hold; 2 = fix as written may not apply.
set -u
set -o pipefail

ROOT="${1:-.}"
CR="$ROOT/crates/runtime_core/src"
VERIFIER="$CR/orchestrator_v2/verifier.rs"
SCHED="$CR/orchestrator_v2/scheduler.rs"
TM="$CR/task_manager.rs"
CMDM="$CR/cmd_match.rs"
EMARK="$CR/evidence_markers.rs"
EPATH="$CR/evidence_paths.rs"
HALT="$CR/halt_gate.rs"

FAIL=0
sec()  { printf '\n== %s ==\n' "$*"; }
ok()   { printf '  [OK]   %s\n' "$*"; }
warn() { printf '  [WARN] %s\n' "$*"; }
bad()  { printf '  [FAIL] %s\n' "$*"; FAIL=$((FAIL+1)); }
info() { printf '  [INFO] %s\n' "$*"; }

anchor() { # file desc regex [want_count]
  local f=$1 desc=$2 pat=$3 want=${4:-1} n lines
  [ -f "$f" ] || { bad "$desc: missing $f"; return; }
  n=$(grep -cE -- "$pat" "$f" 2>/dev/null || true)
  if [ "$n" -eq 0 ]; then bad "$desc: anchor not found in $f  /$pat/"; return; fi
  if [ "$n" -gt "$want" ]; then
    warn "$desc: matched $n times in $f (want $want)"
    grep -nE -- "$pat" "$f" | head -n 4 | sed 's/^/         /'
    return
  fi
  ok "$desc: $f:$(grep -nE -- "$pat" "$f" | head -n1 | cut -d: -f1)"
}

trace() { # regex sym  — read-only definition + call-site census
  local sym=$1 defs calls
  defs=$(grep -rnE -- "fn[[:space:]]+${sym}[[:space:]]*[(<]" "$CR" 2>/dev/null || true)
  calls=$(grep -rnE -- "\b${sym}\b" "$CR" 2>/dev/null | grep -vE "fn[[:space:]]+${sym}\b" || true)
  info "trace: $sym"
  if [ -z "$defs" ]; then info "  def: (none found — external or macro)"; else printf '%s\n' "$defs" | sed 's/^/         def /'; fi
  if [ -z "$calls" ]; then info "  calls: (none)"; else printf '%s\n' "$calls" | sed 's/^/         call /'; fi
}

sec "Phase 0 — environment"
for f in "$VERIFIER" "$SCHED" "$TM" "$CMDM" "$EMARK" "$EPATH" "$HALT"; do
  [ -f "$f" ] && ok "present $f" || bad "missing $f"
done
command -v grep >/dev/null && ok "grep available" || bad "grep missing"

sec "Phase 1 — trace untraced callees (resolves Node 2/3 UNRESOLVED)"
trace "command_condition_met"
trace "strip_workspace_cd_prefix"
trace "cmd_ok_binds_command"
trace "search_term_satisfied"
trace "search_checked_satisfied"
trace "write_path_satisfied"
trace "patch_path_satisfied"
trace "read_function_marker_satisfies"
trace "has_patch_satisfied_marker"
trace "halt_range_gate"
trace "process_verification"

sec "Phase 2 — Fix 1: CMD_OK:<cmd> arm + task_manager call-site"
anchor "$VERIFIER" "verifier CMD_OK arm"        'criterion[[:space:]]*==[[:space:]]*"CMD_OK"'
anchor "$VERIFIER" "verifier cmd_ok_binds_command usage" 'cmd_ok_binds_command'
anchor "$TM"       "task_manager command_ok_for post-check" 'command_ok_for[[:space:]]*\('
anchor "$TM"       "task_manager verify_contract call"     'Verifier::verify_contract[[:space:]]*\('
if grep -qE 'strip_prefix\("CMD_OK:"\)' "$VERIFIER"; then
  warn "verifier already handles CMD_OK:<cmd> — Fix 1 may be partially applied"
else
  ok "verifier has no CMD_OK:<cmd> arm yet (fix 1 applies)"
fi

sec "Phase 3 — Fix 2: normalization alignment"
anchor "$CMDM" "cmd_match module present" 'fn[[:space:]]+(command_condition_met|strip_workspace_cd_prefix)'
anchor "$VERIFIER" "command_ok_for normalizer" 'strip_workspace_cd_prefix'
if grep -qE 'fn[[:space:]]+norm\b' "$CMDM"; then warn "cmd_match already has a norm() helper"; else ok "cmd_match has no norm() helper yet"; fi

sec "Phase 4 — Fix 3: verifier-only criteria"
for crit in SOURCE_CODE_EVIDENCE_REQUIRED MIN_BELIEF_SCORE MATHEMATICAL_EXECUTION_VERIFIED; do
  if grep -qE "\"$crit\"" "$VERIFIER"; then warn "verifier already handles $crit"; else ok "verifier lacks $crit (add arm)"; fi
done
if grep -qE 'fn[[:space:]]+verify_contract\([^)]*belief' "$VERIFIER"; then
  warn "verify_contract already takes belief — MIN_BELIEF_SCORE may be wired"
else
  info "verify_contract signature does NOT take belief — threading required for MIN_BELIEF_SCORE"
fi

sec "Phase 5 — Fix 4: bare-marker alignment"
anchor "$EMARK" "has_patch_satisfied_marker exists" 'fn[[:space:]]+has_patch_satisfied_marker'
anchor "$VERIFIER" "bare PATCH_FUNCTION_OK arm" 'criterion[[:space:]]*==[[:space:]]*"PATCH_FUNCTION_OK"'
for crit in READ_MARKER_ACQUIRED SEARCH_MARKER_ACQUIRED WRITE_MARKER_ACQUIRED; do
  grep -qE "\"$crit\"" "$VERIFIER" && warn "verifier already handles $crit" || ok "verifier lacks $crit (add arm)"
done

sec "Phase 6 — Fix 5: Scheduler::process_verification surface"
anchor "$SCHED" "process_verification definition" 'fn[[:space:]]+process_verification'
if grep -qE 'pub[[:space:]]+fn[[:space:]]+process_verification' "$SCHED"; then
  warn "process_verification is pub — restrict to pub(crate) or add internal verify"
else
  ok "process_verification is already non-pub"
fi

sec "Phase 7 — proposed edits (NOT applied)"
cat <<'EOF'
--- Fix 1 (verifier.rs) — insert BEFORE `criterion == "CMD_OK"` arm ---
} else if let Some(cmd) = criterion.strip_prefix("CMD_OK:") {
    let own = evidence
        .rfind("[DAG LINEAGE COMPLETE sha256:")
        .and_then(|i| evidence[i..].find('\n').map(|j| &evidence[i + j + 1..]))
        .unwrap_or(evidence);
    let reject_after_ok = match (own.rfind("[HALT_REJECTED"), own.rfind("[CMD_OK]")) {
        (Some(r), Some(o)) if r > o => true,
        (Some(_), None) => true,
        _ => false,
    };
    !reject_after_ok && crate::cmd_match::command_condition_met(cmd.trim(), own)
} else if criterion == "CMD_OK" {

--- Fix 1 (task_manager.rs) — replace the command_ok_for post-check block ---
// before:
//   if let Some(cmd) = extract_required_command(&node.objective) {
//       if !Verifier::command_ok_for(&execution_evidence, &cmd) { result = Failure(...) }
//   }
// after:
   let mut criteria_for_verify = verification_criteria.clone();
   if let Some(cmd) = extract_required_command(&node.objective) {
       criteria_for_verify.push(format!("CMD_OK:{}", cmd));
   }
   let result = Verifier::verify_contract(&task_id, &execution_evidence, &criteria_for_verify);

--- Fix 2 (cmd_match.rs) — add normalizer, use in command_condition_met ---
fn norm(c: &str) -> String {
    crate::cmd_match::strip_workspace_cd_prefix(c.trim())
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

--- Fix 3 (verifier.rs) — new arms before final `else { Escalate(...) }` ---
} else if criterion == "SOURCE_CODE_EVIDENCE_REQUIRED" {
    crate::evidence_markers::source_paths(evidence).next().is_some()
} else if let Some(raw) = criterion.strip_prefix("MIN_BELIEF_SCORE:") {
    raw.trim().parse::<f32>().map(|m| belief >= m).unwrap_or(false)
} else if let Some(cmd) = criterion.strip_prefix("MATHEMATICAL_EXECUTION_VERIFIED:") {
    crate::cmd_match::command_condition_met(cmd.trim(), evidence)
        || (crate::completion_engine::math_command_satisfied(evidence)
            && crate::evidence_paths::cmd_ok_binds_command(evidence))

--- Fix 4 (verifier.rs) — widen bare PATCH_FUNCTION_OK ---
} else if criterion == "PATCH_FUNCTION_OK" {
    crate::evidence_markers::has_patch_satisfied_marker(evidence)

--- Fix 5 (scheduler.rs) — close the injectable Success surface ---
pub(crate) fn process_verification(
    graph: &mut TaskGraph, task_id: &str, evidence: &str, criteria: &[String],
) {
    let result = crate::orchestrator_v2::verifier::Verifier::verify_contract(
        task_id, evidence, criteria,
    );
    /* existing match on result unchanged */
}
EOF

sec "Summary"
[ "$FAIL" -eq 0 ] && { ok "all preconditions hold — safe to apply edits in Phase 7"; exit 0; }
bad "$FAIL precondition failure(s) — adjust anchors or resolve UNRESOLVED before editing"; exit 2
