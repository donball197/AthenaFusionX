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
echo "=== HALT diagnostics + fracture guard + matcher hardening | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

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
import difflib, os, pathlib, re, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
OV   = os.environ["OV_REL"]
CL   = os.environ["CL_REL"]
TM   = os.environ["TM_REL"]
EM   = os.environ["EM_REL"]

files = {rel: (root / rel, (root / rel).read_text(encoding="utf-8")) for rel in (OV, CL, TM, EM)}

problems = []

# ---- outcome_verifier.rs: unmet_conditions --------------------------------
OV_ANCHOR = '''    pub fn mission_completion_authorized(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> bool {
        Self::evaluate_contract(local_evidence, belief_score, conditions).is_ok()
    }'''

OV_NEW = OV_ANCHOR + '''

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
                    std::slice::from_ref(*cond),
                )
            })
            .map(|cond| format!("{:?}", cond))
            .collect()
    }'''

OV_TEST_ANCHOR = '''    #[test]
    fn mission_completion_authorized_mirrors_approved_contract() {'''

OV_TEST_INSERT = '''    #[test]
    fn unmet_conditions_names_each_unsatisfied_condition() {
        let conditions = vec![
            CompletionCondition::ReadMarkerAcquired,
            CompletionCondition::WriteMarkerAcquired,
        ];
        let unmet = OutcomeVerifier::unmet_conditions(
            "[READ_OK] crates/runtime_core/src/lib.rs\\n",
            1.0,
            &conditions,
        );
        assert_eq!(unmet.len(), 1);
        assert!(unmet[0].contains("WriteMarkerAcquired"), "got {:?}", unmet);
    }

'''

# ---- cortical_loop.rs: regex-based block matching -------------------------
# Match the whole HALT-rejection block structurally. [^}]* handles the
# `self.log(...)`, `session.local_evidence.push_str(...)` calls without
# needing to reproduce escape sequences (\x1b, \n) exactly.
CL_PATTERN = re.compile(
    r'(?P<indent>[ ]*)if is_halt_action && !deterministic_contract_satisfied \{'
    r'(?P<body>.*?)'
    r'(?P<indent2>[ ]*)return Ok\(CortexState::Strategize\);\n'
    r'(?P=indent)\}',
    re.DOTALL,
)

CL_LOG_ANCHOR = '''        let mut state = CortexState::Strategize;
        let mut turn = 0;'''

CL_LOG_NEW = '''        self.log(format!(
            "\\x1b[90m  -> Contract: read_only={} mutation={} artifacts={:?}\\n  -> Conditions: {:?}\\x1b[0m",
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

def build_cl_replacement(match):
    indent = match.group("indent")
    indent_inner = indent + "    "
    indent_deeper = indent + "        "
    return (
        f'{indent}if is_halt_action && !deterministic_contract_satisfied {{\n'
        f'{indent_inner}let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(\n'
        f'{indent_deeper}&session.local_evidence,\n'
        f'{indent_deeper}current_belief,\n'
        f'{indent_deeper}&session.contract.conditions,\n'
        f'{indent_inner});\n'
        f'{indent_inner}self.log(format!(\n'
        f'{indent_deeper}"\\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Unmet: {{:?}}\\x1b[0m",\n'
        f'{indent_deeper}unmet\n'
        f'{indent_inner}));\n'
        f'{indent_inner}session.local_evidence.push_str(&format!(\n'
        f'{indent_deeper}"\\n[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet: {{:?}}\\n",\n'
        f'{indent_deeper}unmet\n'
        f'{indent_inner}));\n'
        f'{indent_inner}return Ok(CortexState::Strategize);\n'
        f'{indent}}}'
    )

# ---- task_manager.rs: fracture guard over ALL artifacts -------------------
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

# ---- evidence_markers.rs --------------------------------------------------
EM_SPLIT_OLD = '''    let (path_part, term_part) = match expected.rfind(':') {
        Some(c) => (expected[..c].trim().to_string(), expected[c + 1..].trim().to_string()),
        None => (String::new(), expected.clone()),
    };'''

EM_SPLIT_NEW = '''    let (path_part, term_part) = match expected.split_once(':') {
        Some((p, t)) => (p.trim().to_string(), t.trim().to_string()),
        None => (String::new(), expected.clone()),
    };'''

EM_BIDIR_OLD = '''        path_ok && (recorded_query.contains(&term_part) || term_part.contains(&recorded_query))'''

EM_BIDIR_NEW = '''        path_ok && recorded_query.contains(&term_part)'''

# ---- preflight ------------------------------------------------------------
def want1(rel, name, text):
    c = files[rel][1].count(text)
    if c == 0:
        problems.append(f"{name} ANCHOR NOT FOUND in {rel}")
    elif c > 1:
        problems.append(f"{name} ANCHOR x{c} in {rel} (want 1)")

want1(OV, "OV mission_completion_authorized", OV_ANCHOR)
want1(OV, "OV test anchor", OV_TEST_ANCHOR)
want1(TM, "TM fracture guard", TM_OLD)
want1(EM, "EM colon split", EM_SPLIT_OLD)
want1(EM, "EM bidirectional", EM_BIDIR_OLD)

# CL: regex
cl_matches = list(CL_PATTERN.finditer(files[CL][1]))
if len(cl_matches) == 0:
    problems.append("CL HALT rejection: regex did not match any block")
elif len(cl_matches) > 1:
    problems.append(f"CL HALT rejection: regex matched {len(cl_matches)} blocks (want 1)")

want1(CL, "CL state banner", CL_LOG_ANCHOR)

if "unmet_conditions_names_each_unsatisfied_condition" in files[OV][1]:
    problems.append("TEST ALREADY PRESENT: unmet_conditions_names_each_unsatisfied_condition in OV")

for rel, label, text in (
    (OV, "case_8", "fn case_8_read_path_acquired_rejects_wrong_path()"),
    (OV, "lineage tamper", "fn tampered_lineage_payload_fails_verification()"),
    (CL, "write_phase_ready", "fn write_phase_ready("),
    (TM, "critical path", "compute_critical_path_priorities"),
):
    if text not in files[rel][1]:
        problems.append(f"INVARIANT VIOLATED: {label} absent in {rel}")

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for p in problems:
        print("  -", p)
    sys.exit(1)

print("[PREFLIGHT OK] OV/TM/EM anchors, CL regex, CL banner all matched")

# ---- transform -----------------------------------------------------------
ov_p = files[OV][1].replace(OV_ANCHOR, OV_NEW, 1)
ov_p = ov_p.replace(OV_TEST_ANCHOR, OV_TEST_INSERT + OV_TEST_ANCHOR, 1)

cl_p = CL_PATTERN.sub(lambda m: build_cl_replacement(m), files[CL][1], count=1)
cl_p = cl_p.replace(CL_LOG_ANCHOR, CL_LOG_NEW, 1)

tm_p = files[TM][1].replace(TM_OLD, TM_NEW, 1)

em_p = files[EM][1].replace(EM_SPLIT_OLD, EM_SPLIT_NEW, 1)
em_p = em_p.replace(EM_BIDIR_OLD, EM_BIDIR_NEW, 1)

for rel, p in ((OV, ov_p), (CL, cl_p), (TM, tm_p), (EM, em_p)):
    if p == files[rel][1]:
        print(f"[PREFLIGHT FAILED] no change produced in {rel}")
        sys.exit(1)

checks = [
    ("OV unmet_conditions fn", ov_p.count("pub fn unmet_conditions("), 1),
    ("OV unmet test", ov_p.count("fn unmet_conditions_names_each_unsatisfied_condition()"), 1),
    ("CL Unmet log line", cl_p.count("Unmet: {:?}"), 2),
    ("CL unmet_conditions call", cl_p.count("OutcomeVerifier::unmet_conditions("), 1),
    ("TM all-artifact loop", tm_p.count("for expected_path in &expected_paths"), 1),
    ("EM split_once", em_p.count("expected.split_once(':')"), 1),
    ("EM bidir removed", em_p.count("term_part.contains(&recorded_query)"), 0),
    ("EM forward kept", em_p.count("path_ok && recorded_query.contains(&term_part)"), 1),
]
failed = [(n, got, want) for (n, got, want) in checks if got != want]
if failed:
    print("[POST-PATCH FAILED]:")
    for (n, got, want) in failed:
        print(f"  - {n}: got {got}, want {want}")
    sys.exit(1)

print("[POST-PATCH OK] all invariants held")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
for rel, p in ((OV, ov_p), (CL, cl_p), (TM, tm_p), (EM, em_p)):
    sys.stdout.writelines(
        difflib.unified_diff(
            files[rel][1].splitlines(keepends=True),
            p.splitlines(keepends=True),
            fromfile=f"a/{rel}",
            tofile=f"b/{rel}",
        )
    )

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    sys.exit(0)

for rel, p in ((OV, ov_p), (CL, cl_p), (TM, tm_p), (EM, em_p)):
    files[rel][0].write_text(p, encoding="utf-8")
    print(f"[PATCH_OK] {rel}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete."
else
    echo "[OK] HALT diagnostics applied."
    echo "     cargo test -p nexus_runtime_core"
fi
