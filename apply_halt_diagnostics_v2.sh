#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== HALT diagnostics v2 (state-aware) | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

OV="crates/runtime_core/src/outcome_verifier.rs"
CL="crates/runtime_core/src/cortical_loop.rs"
TM="crates/runtime_core/src/task_manager.rs"
EM="crates/runtime_core/src/evidence_markers.rs"
for f in "$OV" "$CL" "$TM" "$EM"; do
    if [[ ! -f "$WORKSPACE_ROOT/$f" ]]; then
        echo "[PREFLIGHT FAILED] missing $f" >&2
        exit 1
    fi
done

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
OV_REL="$OV" CL_REL="$CL" TM_REL="$TM" EM_REL="$EM" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
OV   = os.environ["OV_REL"]
CL   = os.environ["CL_REL"]
TM   = os.environ["TM_REL"]
EM   = os.environ["EM_REL"]

files = {rel: (root / rel, (root / rel).read_text(encoding="utf-8")) for rel in (OV, CL, TM, EM)}
patched = {rel: files[rel][1] for rel in files}

# ============================ anchors =====================================
# All anchors are RAW strings: Rust sources store single backslashes and any
# \x / \n escape in a non-raw Python string silently stops matching.

OV_UNMET_OLD = '''    pub fn mission_completion_authorized(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> bool {
        Self::evaluate_contract(local_evidence, belief_score, conditions).is_ok()
    }'''

OV_UNMET_NEW = OV_UNMET_OLD + '''

    /// Diagnostic helper: names each contract condition that is NOT yet
    /// satisfied. Backs actionable HALT-rejection logging.
    pub fn unmet_conditions(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> Vec<String> {
        conditions
            .iter()
            .filter(|cond| {
                !Self::mission_completion_authorized(
                    local_evidence,
                    belief_score,
                    std::slice::from_ref(cond),
                )
            })
            .map(|cond| format!("{:?}", cond))
            .collect()
    }'''

OV_TEST_OLD = '''    #[test]
    fn mission_completion_authorized_mirrors_approved_contract() {'''

OV_TEST_NEW = '''    #[test]
    fn unmet_conditions_names_each_unsatisfied_condition() {
        let conditions = vec![
            CompletionCondition::ReadMarkerAcquired,
            CompletionCondition::WriteMarkerAcquired,
        ];
        let unmet = OutcomeVerifier::unmet_conditions(
            "[READ_OK] crates/runtime_core/src/lib.rs",
            1.0,
            &conditions,
        );
        assert_eq!(unmet.len(), 1);
        assert!(unmet[0].contains("WriteMarkerAcquired"), "got {:?}", unmet);
    }

    #[test]
    fn mission_completion_authorized_mirrors_approved_contract() {'''

CL_REJECT_OLD = r'''                if is_halt_action && !deterministic_contract_satisfied {
                    self.log(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Continuing mission.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "\n[HALT_REJECTED] Deterministic completion contract not satisfied.\n",
                    );
                    return Ok(CortexState::Strategize);
                }'''

CL_REJECT_NEW = r'''                if is_halt_action && !deterministic_contract_satisfied {
                    let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                        &session.local_evidence,
                        current_belief,
                        &session.contract.conditions,
                    );
                    self.log(format!(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Unmet: {:?}\x1b[0m",
                        unmet
                    ));
                    session.local_evidence.push_str(&format!(
                        "\n[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet: {:?}\n",
                        unmet
                    ));
                    return Ok(CortexState::Strategize);
                }'''

CL_LOG_OLD = '''        let mut state = CortexState::Strategize;
        let mut turn = 0;'''

CL_LOG_NEW = r'''        self.log(format!(
            "\x1b[90m  -> Contract: read_only={} mutation={} artifacts={:?}\n  -> Conditions: {:?}\x1b[0m",
            session.contract.read_only,
            session.contract.is_mutation_task,
            session.contract
                .required_artifact_paths
                .iter()
                .map(|a| &a.path)
                .collect::<Vec<_>>(),
            session.contract.conditions,
        ));

        let mut state = CortexState::Strategize;
        let mut turn = 0;'''

TM_OLD = '''    if let Some(expected_path) =
        crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", raw_directive)
            .required_artifact_paths
            .first()
    {
        let has_write_node =
            graph.nodes.values().any(|n| n.objective.contains(&expected_path.path));
        if !has_write_node {
            return format!(
                "FRACTURE: Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                expected_path.path
            );
        }
    }'''

TM_NEW = '''    let expected_paths: Vec<String> =
        crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", raw_directive)
            .required_artifact_paths
            .into_iter()
            .map(|a| a.path)
            .collect();
    for expected_path in &expected_paths {
        let has_write_node =
            graph.nodes.values().any(|n| n.objective.contains(expected_path.as_str()));
        if !has_write_node {
            return format!(
                "FRACTURE: Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                expected_path
            );
        }
    }'''

EM_SPLIT_OLD = r'''    let (path_part, term_part) = match expected.rfind(':') {
        Some(c) => (expected[..c].trim().to_string(), expected[c + 1..].trim().to_string()),
        None => (String::new(), expected.clone()),
    };'''

EM_SPLIT_NEW = r'''    // Split the compound criterion on the FIRST colon after the marker: the
    // term may itself contain colons (e.g. "SEARCH_CODE_OK:src:state:machine").
    // rfind(':') swallowed the term's head into the path.
    let (path_part, term_part) = match expected.split_once(':') {
        Some((p, t)) => (p.trim().to_string(), t.trim().to_string()),
        None => (String::new(), expected.clone()),
    };'''

EM_BIDIR_OLD = r'''        path_ok && (recorded_query.contains(&term_part) || term_part.contains(&recorded_query))'''

EM_BIDIR_NEW = r'''        // One-directional: the recorded query must contain the criterion term.
        // The reverse direction let a SHORT query satisfy a LONGER criterion.
        path_ok && recorded_query.contains(&term_part)'''

# ====================== targets: (key, rel, label, old, new, already-probe,
#                      final-probe, region-probe, region-span) =============
targets = [
    ("OV_UNMET",  OV, "outcome_verifier: unmet_conditions helper", OV_UNMET_OLD, OV_UNMET_NEW,
     "pub fn unmet_conditions(", "pub fn unmet_conditions(", "pub fn mission_completion_authorized", 25),
    ("OV_TEST",   OV, "outcome_verifier: unmet_conditions test", OV_TEST_OLD, OV_TEST_NEW,
     "fn unmet_conditions_names_each_unsatisfied_condition", "fn unmet_conditions_names_each_unsatisfied_condition",
     "fn mission_completion_authorized_mirrors_approved_contract", 8),
    ("CL_REJECT", CL, "cortical_loop: HALT rejection detail", CL_REJECT_OLD, CL_REJECT_NEW,
     "[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet",
     "[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet",
     "deterministic completion contract is not satisfied", 25),
    ("CL_LOG",    CL, "cortical_loop: contract ignition log", CL_LOG_OLD, CL_LOG_NEW,
     "  -> Conditions: {:?}", "  -> Conditions: {:?}",
     "let mut state = CortexState::Strategize;", 8),
    ("TM_GUARD",  TM, "task_manager: all-artifact fracture guard", TM_OLD, TM_NEW,
     "for expected_path in &expected_paths", "for expected_path in &expected_paths",
     "FRACTURE: Directive requires writing", 18),
    ("EM_SPLIT",  EM, "evidence_markers: first-colon criterion split", EM_SPLIT_OLD, EM_SPLIT_NEW,
     "expected.split_once(':')", "expected.split_once(':')",
     "pub fn search_term_satisfied", 60),
    ("EM_BIDIR",  EM, "evidence_markers: one-directional term match", EM_BIDIR_OLD, EM_BIDIR_NEW,
     "path_ok && recorded_query.contains(&term_part)", "path_ok && recorded_query.contains(&term_part)",
     "pub fn search_term_satisfied", 60),
]

def dump_region(rel, probe, span):
    lines = files[rel][1].splitlines()
    idx = next((i for i, ln in enumerate(lines) if probe in ln), None)
    if idx is None:
        print(f"    [REGION DUMP FAILED] probe not found in {rel}: {probe!r}")
        return
    lo, hi = max(0, idx - span), min(len(lines), idx + span)
    print(f"    [REGION] {rel} around line {idx + 1} (probe: {probe!r}):")
    for i in range(lo, hi):
        print(f"      {i + 1:5d} | {lines[i]}")

plan = []
failures = []

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    text = patched[rel]
    old_count = text.count(old)
    if already in text:
        status = "SKIP (already present)"
        plan.append((key, rel, None))
    elif old_count == 1:
        status = "APPLY"
        plan.append((key, rel, (old, new)))
    elif old_count > 1:
        status = f"FAIL (anchor x{old_count}, want exactly 1)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    else:
        status = "FAIL (anchor not found)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    print(f"[STATUS] {key:9s} {label:48s} : {status}")

# invariants that must survive
for rel, label, text in (
    (OV, "case_8 regression", "fn case_8_read_path_acquired_rejects_wrong_path()"),
    (OV, "lineage tamper regression", "fn tampered_lineage_payload_fails_verification()"),
    (CL, "write_phase_ready", "fn write_phase_ready("),
    (TM, "critical path", "compute_critical_path_priorities"),
    (EM, "search_term_satisfied", "pub fn search_term_satisfied("),
):
    if text not in files[rel][1]:
        print(f"[INVARIANT VIOLATED] {label} is absent in {rel}")
        failures.append(("INVARIANT", rel, label, text, 5))

if failures:
    print()
    print("[DIAGNOSTIC] region dumps for failed targets:")
    for key, rel, label, region_probe, span in failures:
        print(f"  -- {key}: {label}")
        dump_region(rel, region_probe, span)
    print()
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED. Fix anchors or confirm targets were already applied, then re-run.")
    sys.exit(1)

# ------------------------- transformations -------------------------------
changed = []
for key, rel, edit in plan:
    if edit is None:
        continue
    old, new = edit
    patched[rel] = patched[rel].replace(old, new, 1)
    changed.append((key, rel))

if not changed:
    print()
    print("[NO-OP] every target is already in its final form. Nothing to write.")
    sys.exit(0)

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    if final_probe not in patched[rel]:
        print(f"[POST-PATCH FAILED] {label}: final probe absent: {final_probe!r}")
        sys.exit(1)
if "term_part.contains(&recorded_query)" in patched[EM]:
    print("[POST-PATCH FAILED] EM_BIDIR: bidirectional match still present")
    sys.exit(1)

print()
print(f"[POST-PATCH OK] {len(changed)} target(s) transformed: {', '.join(k for k, _ in changed)}")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
for rel in (OV, CL, TM, EM):
    if patched[rel] != files[rel][1]:
        sys.stdout.writelines(
            difflib.unified_diff(
                files[rel][1].splitlines(keepends=True),
                patched[rel].splitlines(keepends=True),
                fromfile=f"a/{rel}",
                tofile=f"b/{rel}",
            )
        )

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

for rel in (OV, CL, TM, EM):
    if patched[rel] != files[rel][1]:
        files[rel][0].write_text(patched[rel], encoding="utf-8")
        print(f"[PATCH_OK] {rel}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the status lines and diff above."
else
    echo "[OK] HALT diagnostics applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib outcome_verifier"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
