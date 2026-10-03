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
echo "=== declared-criteria authority | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

CE="crates/runtime_core/src/completion_engine.rs"
if [[ ! -f "$WORKSPACE_ROOT/$CE" ]]; then
    echo "[PREFLIGHT FAILED] missing $CE" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
CE_REL="$CE" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
CE   = os.environ["CE_REL"]

path     = root / CE
original = path.read_text(encoding="utf-8")

IMPL_ANCHOR = '''impl SubgoalCompletionEngine {
    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {'''

HELPERS = '''/// Structural scopes of a composed DAG-node mission.
pub(crate) struct MissionScopes<'a> {
    pub objective: &'a str,
    pub criteria: &'a str,
    pub root_directive: &'a str,
    pub lineage: &'a str,
}

impl<'a> MissionScopes<'a> {
    pub fn parse(raw: &'a str) -> Self {
        const CRITERIA_HEADER: &str = "MANDATORY NODE SUCCESS CRITERIA:";
        const ROOT_HEADER: &str = "ROOT DIRECTIVE CONTEXT:";
        const LINEAGE_HEADER: &str = "LINEAGE CONTEXT:";

        let criteria_start = raw.find(CRITERIA_HEADER);
        let root_start = raw.find(ROOT_HEADER);
        let lineage_start = raw.find(LINEAGE_HEADER);

        if criteria_start.is_none() && root_start.is_none() {
            return Self { objective: raw, criteria: raw, root_directive: "", lineage: "" };
        }

        let objective_end = criteria_start.or(root_start).unwrap_or(raw.len());
        let objective = &raw[..objective_end];

        let criteria_end = root_start.or(lineage_start).unwrap_or(raw.len());
        let mut criteria = match criteria_start {
            Some(s) => &raw[s + CRITERIA_HEADER.len()..criteria_end],
            None => "",
        };
        if let Some(rule_start) = criteria.find("NODE COMPLETION RULE") {
            criteria = &criteria[..rule_start];
        }

        let root_directive = match (root_start, lineage_start) {
            (Some(rs), Some(ls)) if ls > rs => &raw[rs + ROOT_HEADER.len()..ls],
            (Some(rs), _) => &raw[rs + ROOT_HEADER.len()..],
            _ => "",
        };
        let lineage = match lineage_start {
            Some(ls) => &raw[ls + LINEAGE_HEADER.len()..],
            None => "",
        };

        Self { objective, criteria, root_directive, lineage }
    }

    pub fn inference_text(&self) -> String {
        if std::ptr::eq(self.objective, self.criteria) {
            return self.objective.to_string();
        }
        format!("{}\\n{}", self.objective, self.criteria)
    }
}

fn detect_read_only(text: &str) -> bool {
    let lower = text.to_lowercase();
    const PHRASES: &[&str] = &[
        "read-only",
        "read only",
        "audit only",
        "do not modify any",
        "don't modify any",
        "without modifying any",
        "do not modify files",
        "don't modify files",
        "without modifying files",
        "do not modify, create",
        "do not modify/create",
        "do not write any",
        "don't write any",
        "without writing any",
        "do not create any",
        "don't create any",
        "do not change any",
        "don't change any",
        "no files may be modified",
        "must not be modified",
        "must not modify",
    ];
    PHRASES.iter().any(|p| lower.contains(p))
}

fn declared_criteria_contract(
    node_id: &str,
    text: &str,
    read_only: bool,
) -> Option<SubgoalContract> {
    let mut conditions: Vec<CompletionCondition> = Vec::new();
    let mut required_artifact_paths: Vec<ArtifactSpec> = Vec::new();

    for path in extract_all_named_targets(text, "READ_OK:") {
        conditions.push(CompletionCondition::ReadPathAcquired { path });
    }

    for path in extract_all_named_targets(text, "WRITE_OK:") {
        if read_only {
            continue;
        }
        conditions.push(CompletionCondition::WritePathAcquired { path: path.clone() });
        required_artifact_paths.push(ArtifactSpec { path, reason: "output_deliverable".into() });
    }

    if (text.contains("PATCH_FUNCTION_OK:")
        || text.contains("DIFF_PATCH_OK:")
        || text.contains("REPLACE_LINES_OK:"))
        && !read_only
    {
        conditions.push(CompletionCondition::PatchMarkerAcquired);
    }

    for term in extract_all_named_targets(text, "SEARCH_CODE_OK:") {
        conditions.push(CompletionCondition::SearchTermAcquired { term });
    }

    if text.contains("SEARCH_MEMORY_OK") {
        conditions.push(CompletionCondition::SearchMemoryMarkerAcquired);
    }

    if text.contains("DAG_LINEAGE_OK") || text.contains("LINEAGE MODE: INHERIT_ONLY") {
        conditions.push(CompletionCondition::LineageEvidenceAcquired);
    }

    if let Some(pos) = text.find("READ_FUNCTION_OK:") {
        let remainder = &text[pos + "READ_FUNCTION_OK:".len()..];
        let target =
            remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');
        let mut parts = target.splitn(2, ':');
        let path = parts.next().unwrap_or("").trim().to_string();
        let function_name = parts.next().unwrap_or("").trim().to_string();
        if !path.is_empty() && !function_name.is_empty() {
            conditions
                .push(CompletionCondition::ReadFunctionMarkerAcquired { path, function_name });
        }
    }

    if text.contains("CMD_OK") {
        if let Some(command) = extract_required_command(text) {
            conditions.push(CompletionCondition::CommandExecutionVerified { command });
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
        }
    }

    if conditions.is_empty() {
        return None;
    }

    let only_search_markers = conditions.iter().all(|c| {
        matches!(
            c,
            CompletionCondition::SearchTermAcquired { .. }
                | CompletionCondition::SearchMarkerAcquired
                | CompletionCondition::SearchMemoryMarkerAcquired
        )
    });
    let has_output = conditions.iter().any(|c| {
        matches!(
            c,
            CompletionCondition::WritePathAcquired { .. }
                | CompletionCondition::WriteMarkerAcquired
                | CompletionCondition::PatchMarkerAcquired
        )
    });
    let has_recon = conditions.iter().any(|c| {
        matches!(
            c,
            CompletionCondition::ReadPathAcquired { .. }
                | CompletionCondition::ReadMarkerAcquired
                | CompletionCondition::ReadFunctionMarkerAcquired { .. }
                | CompletionCondition::SearchTermAcquired { .. }
                | CompletionCondition::SearchMarkerAcquired
                | CompletionCondition::SearchMemoryMarkerAcquired
                | CompletionCondition::SourceCodeEvidenceRequired
        )
    });

    if has_recon && !only_search_markers {
        let threshold = if has_output { 0.75 } else { 0.65 };
        conditions.push(CompletionCondition::MinBeliefScore(threshold));
    }

    Some(SubgoalContract {
        node_id: node_id.to_string(),
        objective: text.to_string(),
        is_mutation_task: has_output,
        read_only,
        conditions,
        required_artifact_paths,
    })
}

'''

PROLOGUE_OLD = '''        let obj_lower = objective.trim().to_lowercase();
        let classify_text = obj_lower
            .strip_prefix("objective:")
            .or_else(|| obj_lower.strip_prefix("objective"))
            .unwrap_or(&obj_lower)
            .trim();

        let mut conditions = Vec::new();
        let mut is_mutation_task = false;'''

PROLOGUE_NEW = '''        let scopes = MissionScopes::parse(objective);
        let inference_text = scopes.inference_text();
        let objective: &str = &inference_text;

        let obj_lower = objective.trim().to_lowercase();
        let classify_text = obj_lower
            .strip_prefix("objective:")
            .or_else(|| obj_lower.strip_prefix("objective"))
            .unwrap_or(&obj_lower)
            .trim();

        let mut conditions = Vec::new();
        let mut is_mutation_task = false;'''

READ_ONLY_OLD = '''        let read_only = classify_text.contains("read-only")
            || classify_text.contains("read only")
            || classify_text.contains("audit only")
            || classify_text.contains("do not modify any source files");'''

READ_ONLY_BROADENED = '''        let read_only = classify_text.contains("read-only")
            || classify_text.contains("read only")
            || classify_text.contains("audit only")
            || classify_text.contains("do not modify any source files")
            || classify_text.contains("do not modify any files")
            || classify_text.contains("do not modify")
            || classify_text.contains("without modifying")
            || classify_text.contains("must not modify");'''

READ_ONLY_NEW = '''        let declared_mutation_criteria = objective.contains("WRITE_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("DIFF_PATCH_OK:")
            || objective.contains("REPLACE_LINES_OK:");
        let read_only = detect_read_only(objective)
            || (detect_read_only(scopes.root_directive) && !declared_mutation_criteria);

        if let Some(contract) = declared_criteria_contract(node_id, objective, read_only) {
            return contract;
        }'''

HAS_WRITE_OLD = '''        let has_write = any_word_unnegated(
            classify_text,
            &["write", "export", "save", "implement", "integrate", "add", "create", "synthesize"],
        );'''

HAS_WRITE_NEW = '''        let has_write = any_word_unnegated(
            classify_text,
            &["write", "export", "save", "implement", "integrate", "create", "synthesize"],
        );'''

TEST_ANCHOR = '''    #[test]
    fn mission2_shape_reads_satisfied_once_the_command_is_evidenced() {'''

TEST_INSERT = '''    fn composed_audit_mission(root_context: &str) -> String {
        format!(
            r#"OBJECTIVE: Perform a deterministic code audit of the provided Rust logic.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
- SEARCH_CODE_OK:crates/runtime_core/src:Evidence

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced.

ROOT DIRECTIVE CONTEXT:
{}

LINEAGE CONTEXT:
[NO COMPLETED OUTPUT AVAILABLE]"#,
            root_context
        )
    }

    #[test]
    fn root_context_output_labels_do_not_mint_mutation_conditions() {
        let mission = composed_audit_mission(
            r#"Then provide exactly this output format:
ROOT_CAUSE: <both independent defects>
TRACE: <...>
FIX: <the minimal logical correction>
Do not modify any files."#,
        );
        let contract = SubgoalCompletionEngine::infer_contract("task_1", &mission);

        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired
                    | CompletionCondition::WriteMarkerAcquired
                    | CompletionCondition::WritePathAcquired { .. }
            )),
            "root context must not mint mutation conditions: {:?}",
            contract.conditions
        );
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term }
                if term == "crates/runtime_core/src:Evidence"
        )));
        assert!(contract.read_only, "root-level 'Do not modify any files.' must set read_only");
    }

    #[test]
    fn failed_audit_run_contract_is_actually_satisfiable() {
        let mission = composed_audit_mission(
            r#"Provide the output format with a FIX: line.
Do not modify any files."#,
        );
        let contract = SubgoalCompletionEngine::infer_contract("task_1", &mission);

        let evidence = r#"[SEARCH_CODE_OK] Path: crates/runtime_core/src | Query: 'Evidence'
MATCHES:
lib.rs:1:enum Evidence
"#;
        assert!(
            SubgoalCompletionEngine::evaluate(&contract, evidence, 0.5),
            "conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn fix_label_in_objective_alongside_declared_criteria_is_inert() {
        let mission = r#"OBJECTIVE: Identify the defects and report FIX: the correction.

MANDATORY NODE SUCCESS CRITERIA:
SEARCH_CODE_OK:src:gate"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired | CompletionCondition::WriteMarkerAcquired
            )),
            "conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn do_not_modify_any_files_sets_read_only() {
        let contract =
            SubgoalCompletionEngine::infer_contract("t", "Audit the report. Do not modify any files.");
        assert!(contract.read_only);
        assert!(contract.required_artifact_paths.is_empty());
    }

    #[test]
    fn lineage_output_cannot_impose_read_only_on_child_node() {
        let mission = r#"OBJECTIVE: Write summary.md of the findings.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:summary.md

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced.

ROOT DIRECTIVE CONTEXT:
Summarize the findings.

LINEAGE CONTEXT:
---
[DEPENDENCY LINEAGE FROM task_0]:
The upstream audit was read-only; do not modify any files.
---
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(!contract.read_only, "lineage text must not impose read-only on the child");
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "summary.md"
        )));
        assert_eq!(contract.required_artifact_paths.len(), 1);
    }

    #[test]
    fn wrapped_command_node_preserves_original_criteria() {
        let mission = r#"OBJECTIVE: Execute `cargo check -p math_utils` and report the actual command used and its output as verifiable evidence.

MANDATORY NODE SUCCESS CRITERIA:
CMD_OK

Then complete the original objective:
READ_OK:crates/runtime_core/src/lib.rs

ROOT DIRECTIVE CONTEXT:
Fix the build failure.

LINEAGE CONTEXT:
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::CommandExecutionVerified { command }
                if command == "cargo check -p math_utils"
        )), "conditions = {:?}", contract.conditions);
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::ReadPathAcquired { path }
                if path == "crates/runtime_core/src/lib.rs"
        )), "conditions = {:?}", contract.conditions);
    }

    #[test]
    fn mission_scopes_exclude_root_and_lineage() {
        let mission = composed_audit_mission("FIX: the correction.\\nDo not modify any files.");
        let scopes = super::MissionScopes::parse(&mission);

        assert!(scopes.inference_text().contains("SEARCH_CODE_OK:"));
        assert!(!scopes.inference_text().contains("FIX:"), "leaked");
        assert!(scopes.root_directive.contains("FIX:"));
    }

    #[test]
    fn write_only_declared_contract_carries_no_belief_threshold() {
        let mission = r#"MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:out.md"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(
            !contract.conditions.iter().any(|c| matches!(c, CompletionCondition::MinBeliefScore(_)))
        );
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "out.md"
        )));
    }

    #[test]
    fn declared_write_criterion_wins_over_root_read_only_restriction() {
        let mission = r#"OBJECTIVE: Produce the summary artifact.

MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:summary.md

ROOT DIRECTIVE CONTEXT:
Do not modify any files.
LINEAGE CONTEXT:
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(!contract.read_only);
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "summary.md"
        )));
    }

'''

PATH_BOUND_INVARIANT = '''        let has_explicit_path_bound = objective.contains("READ_OK:")
            || objective.contains("WRITE_OK:")
            || objective.contains("SEARCH_CODE_OK:")
            || objective.contains("SEARCH_MEMORY_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("READ_FUNCTION_OK:");'''

problems = []

# --- reject if the helper symbols are already present ---
for sym in ("fn declared_criteria_contract(", "pub(crate) struct MissionScopes",
            "fn detect_read_only(", "detect_read_only(objective)"):
    if sym in original:
        problems.append(f"ALREADY APPLIED: '{sym}' already present in {CE}")

def want1(name, text):
    c = original.count(text)
    if c == 0:
        problems.append(f"{name} ANCHOR NOT FOUND")
    elif c > 1:
        problems.append(f"{name} ANCHOR x{c} (want 1)")

want1("IMPL", IMPL_ANCHOR)
want1("PROLOGUE", PROLOGUE_OLD)

ro_source = None
if original.count(READ_ONLY_OLD) == 1:
    ro_source = READ_ONLY_OLD
elif original.count(READ_ONLY_BROADENED) == 1:
    ro_source = READ_ONLY_BROADENED
    print("[NOTE] read_only anchor is the broadened form; superseded by detect_read_only")
else:
    problems.append("READ_ONLY ANCHOR NOT FOUND")

want1("TEST", TEST_ANCHOR)

for name in (
    "root_context_output_labels_do_not_mint_mutation_conditions",
    "failed_audit_run_contract_is_actually_satisfiable",
    "fix_label_in_objective_alongside_declared_criteria_is_inert",
    "do_not_modify_any_files_sets_read_only",
    "lineage_output_cannot_impose_read_only_on_child_node",
    "wrapped_command_node_preserves_original_criteria",
    "mission_scopes_exclude_root_and_lineage",
    "write_only_declared_contract_carries_no_belief_threshold",
    "declared_write_criterion_wins_over_root_read_only_restriction",
):
    if name in original:
        problems.append(f"TEST ALREADY PRESENT: {name}")

for label, text in (
    ("path-bound six-marker guard", PATH_BOUND_INVARIANT),
    ("case_8", "fn case_8_search_code_plus_search_memory_without_reads_preserves_both()"),
    ("case_7", "fn case_7_search_only_contract_requires_search_marker_only()"),
    ("reconnaissance_complete", "pub fn reconnaissance_complete("),
):
    if text not in original:
        problems.append(f"INVARIANT VIOLATED: {label} absent")

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for p in problems:
        print("  -", p)
    sys.exit(1)

print("[PREFLIGHT OK] anchors and invariants verified")

patched = original.replace(IMPL_ANCHOR, HELPERS + IMPL_ANCHOR, 1)
patched = patched.replace(PROLOGUE_OLD, PROLOGUE_NEW, 1)
patched = patched.replace(ro_source, READ_ONLY_NEW, 1)

if patched.count(HAS_WRITE_OLD) == 1:
    patched = patched.replace(HAS_WRITE_OLD, HAS_WRITE_NEW, 1)
    print("[POST-PATCH] has_write: dropped 'add'")
elif HAS_WRITE_NEW in patched:
    print("[ALREADY APPLIED] has_write: 'add' already absent")
else:
    print("[WARN] has_write anchor not found; 'add' removal skipped")

patched = patched.replace(TEST_ANCHOR, TEST_INSERT + TEST_ANCHOR, 1)

if patched == original:
    print("[PREFLIGHT FAILED] no textual change produced")
    sys.exit(1)

# --- explicit post-patch invariants with labels ---
checks = [
    ("declared_criteria_contract fn", patched.count("fn declared_criteria_contract("), 1),
    ("MissionScopes struct", patched.count("pub(crate) struct MissionScopes"), 1),
    ("detect_read_only fn", patched.count("fn detect_read_only("), 1),
    ("READ_ONLY_NEW present", patched.count(READ_ONLY_NEW), 1),
    ("PROLOGUE_NEW present", patched.count(PROLOGUE_NEW), 1),
    ("root_context test present",
     patched.count("fn root_context_output_labels_do_not_mint_mutation_conditions()"), 1),
    ("wrapped_command test present",
     patched.count("fn wrapped_command_node_preserves_original_criteria()"), 1),
    ("path-bound guard present", int(PATH_BOUND_INVARIANT in patched), 1),
    ("case_8 present",
     int("fn case_8_search_code_plus_search_memory_without_reads_preserves_both()" in patched), 1),
    ("case_7 present",
     int("fn case_7_search_only_contract_requires_search_marker_only()" in patched), 1),
    ("recon gate present", int("pub fn reconnaissance_complete(" in patched), 1),
]

failed = [(n, got, want) for (n, got, want) in checks if got != want]
if failed:
    print("[POST-PATCH FAILED] invariants not held:")
    for (n, got, want) in failed:
        print(f"  - {n}: got {got}, want {want}")
    sys.exit(1)

print("[POST-PATCH OK] all invariants held")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
sys.stdout.writelines(
    difflib.unified_diff(
        original.splitlines(keepends=True),
        patched.splitlines(keepends=True),
        fromfile=f"a/{CE}",
        tofile=f"b/{CE}",
    )
)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    sys.exit(0)

path.write_text(patched, encoding="utf-8")
print()
print(f"[PATCH_OK] {CE}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete."
else
    echo "[OK] declared-criteria authority applied."
    echo "     cargo test -p nexus_runtime_core"
fi
