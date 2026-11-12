#!/usr/bin/env bash
# patch_infer_contract_scope.sh — scope marker-string authority to the
# MANDATORY NODE SUCCESS CRITERIA block.
#
# Bug (confirmed by Mission D attempts 1 and 2): completion_engine's
# declared_criteria_contract and the has_explicit_path_bound guard mine
# marker strings from the whole inference_text (objective + criteria).
# A mission whose objective *discusses* a marker -- e.g. an audit that
# says "document what [CMD_OK] means" -- gets that marker promoted into
# an unsatisfiable completion condition.
#
# Fix: pass scopes.criteria to declared_criteria_contract for marker
# mining, but keep the full inference_text available for command
# extraction (extract_required_command), where prose-to-command
# extraction is the documented and tested behavior.
#
# Guard rail: missions with no criteria header parse with
# criteria == raw (MissionScopes::parse), so single-line directives
# like "READ_OK:foo.rs" keep working unchanged.
#
# Usage:
#   ./patch_infer_contract_scope.sh            # dry-run
#   ./patch_infer_contract_scope.sh --apply    # apply + targeted tests
#   ./patch_infer_contract_scope.sh --help

set -euo pipefail

ROOT="$(pwd)"
CE="$ROOT/crates/runtime_core/src/completion_engine.rs"
APPLY=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

[[ -f "$CE" ]] || { echo "ERROR: $CE not found (run from repo root)" >&2; exit 1; }

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$CE.bak.$STAMP"
cp "$CE" "$BACKUP"
echo "backup: $BACKUP"

PATCHED="$(mktemp)"
trap 'rm -f "$PATCHED"' EXIT

python3 - "$CE" "$PATCHED" <<'PY'
import sys

src_path, out_path = sys.argv[1], sys.argv[2]
src = open(src_path, encoding='utf-8').read()

def expect_one(needle, label):
    global src
    n = src.count(needle)
    if n != 1:
        sys.stderr.write(f"ERROR[{label}]: expected 1 match, found {n}\n")
        sys.stderr.write(f"  needle: {needle[:100]!r}\n")
        sys.exit(3)

# ---- 1. declared_criteria_contract signature ----------------------------
expect_one(
    "fn declared_criteria_contract(\n"
    "    node_id: &str,\n"
    "    text: &str,\n"
    "    read_only: bool,\n"
    ") -> Option<SubgoalContract> {",
    "signature"
)
src = src.replace(
    "fn declared_criteria_contract(\n"
    "    node_id: &str,\n"
    "    text: &str,\n"
    "    read_only: bool,\n"
    ") -> Option<SubgoalContract> {",
    "fn declared_criteria_contract(\n"
    "    node_id: &str,\n"
    "    criteria_text: &str,\n"
    "    full_text: &str,\n"
    "    read_only: bool,\n"
    ") -> Option<SubgoalContract> {",
    1,
)

# ---- 2. Swap all `extract_all_named_targets(text, ...)` to criteria_text
for marker in [
    "READ_OK:", "WRITE_OK:", "DIFF_PATCH_OK:", "REPLACE_LINES_OK:",
    "PATCH_FUNCTION_OK:", "SEARCH_CODE_OK:", "SEARCH_CHECKED_OK:",
]:
    old = f'extract_all_named_targets(text, "{marker}")'
    new = f'extract_all_named_targets(criteria_text, "{marker}")'
    expect_one(old, f"targets-{marker}")
    src = src.replace(old, new, 1)

# ---- 3. Swap remaining `text.contains(...)` in declared_criteria_contract
# Only inside declared_criteria_contract — anchor by unique surrounding lines.

# 3a. "text.contains(\"PATCH_FUNCTION_OK:\")" in the fallback-patch check
expect_one(
    "        if !pushed_typed_patch\n"
    "            && (text.contains(\"PATCH_FUNCTION_OK:\")\n"
    "                || text.contains(\"DIFF_PATCH_OK:\")\n"
    "                || text.contains(\"REPLACE_LINES_OK:\"))\n",
    "fallback-patch"
)
src = src.replace(
    "        if !pushed_typed_patch\n"
    "            && (text.contains(\"PATCH_FUNCTION_OK:\")\n"
    "                || text.contains(\"DIFF_PATCH_OK:\")\n"
    "                || text.contains(\"REPLACE_LINES_OK:\"))\n",
    "        if !pushed_typed_patch\n"
    "            && (criteria_text.contains(\"PATCH_FUNCTION_OK:\")\n"
    "                || criteria_text.contains(\"DIFF_PATCH_OK:\")\n"
    "                || criteria_text.contains(\"REPLACE_LINES_OK:\"))\n",
    1,
)

# 3b. SEARCH_MEMORY_OK
expect_one('    if text.contains("SEARCH_MEMORY_OK") {', "sm-ok")
src = src.replace(
    '    if text.contains("SEARCH_MEMORY_OK") {',
    '    if criteria_text.contains("SEARCH_MEMORY_OK") {',
    1,
)

# 3c. DAG_LINEAGE_OK / LINEAGE MODE
expect_one(
    '    if text.contains("DAG_LINEAGE_OK") || text.contains("LINEAGE MODE: INHERIT_ONLY") {',
    "dag-lineage"
)
src = src.replace(
    '    if text.contains("DAG_LINEAGE_OK") || text.contains("LINEAGE MODE: INHERIT_ONLY") {',
    '    if criteria_text.contains("DAG_LINEAGE_OK") || criteria_text.contains("LINEAGE MODE: INHERIT_ONLY") {',
    1,
)

# 3d. READ_FUNCTION_OK: find + slice
expect_one(
    '    if let Some(pos) = text.find("READ_FUNCTION_OK:") {\n'
    '        let remainder = &text[pos + "READ_FUNCTION_OK:".len()..];',
    "read-fn"
)
src = src.replace(
    '    if let Some(pos) = text.find("READ_FUNCTION_OK:") {\n'
    '        let remainder = &text[pos + "READ_FUNCTION_OK:".len()..];',
    '    if let Some(pos) = criteria_text.find("READ_FUNCTION_OK:") {\n'
    '        let remainder = &criteria_text[pos + "READ_FUNCTION_OK:".len()..];',
    1,
)

# 3e. CMD_OK guard -> criteria_text, command extraction -> full_text
expect_one(
    '    if text.contains("CMD_OK") {\n'
    '        if let Some(command) = extract_required_command(text) {',
    "cmd-ok"
)
src = src.replace(
    '    if text.contains("CMD_OK") {\n'
    '        if let Some(command) = extract_required_command(text) {',
    '    if criteria_text.contains("CMD_OK") {\n'
    '        if let Some(command) = extract_required_command(full_text) {',
    1,
)

# 3f. SubgoalContract.objective field must remain full_text (semantic
#     classification depends on seeing the whole mission text).
expect_one(
    "    Some(SubgoalContract {\n"
    "        node_id: node_id.to_string(),\n"
    "        objective: text.to_string(),\n",
    "contract-objective"
)
src = src.replace(
    "    Some(SubgoalContract {\n"
    "        node_id: node_id.to_string(),\n"
    "        objective: text.to_string(),\n",
    "    Some(SubgoalContract {\n"
    "        node_id: node_id.to_string(),\n"
    "        objective: full_text.to_string(),\n",
    1,
)

# ---- 4. declared_mutation_criteria -> scopes.criteria --------------------
expect_one(
    '        let declared_mutation_criteria = objective.contains("WRITE_OK:")\n'
    '            || objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || objective.contains("DIFF_PATCH_OK:")\n'
    '            || objective.contains("REPLACE_LINES_OK:");',
    "declared-mut"
)
src = src.replace(
    '        let declared_mutation_criteria = objective.contains("WRITE_OK:")\n'
    '            || objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || objective.contains("DIFF_PATCH_OK:")\n'
    '            || objective.contains("REPLACE_LINES_OK:");',
    '        // Marker-string authority lives in the criteria block, not in\n'
    '        // the whole mission text. A mission that merely *discusses* a\n'
    '        // marker (e.g. an audit documenting the executor) must not be\n'
    '        // treated as declaring that marker as a requirement.\n'
    '        let declared_mutation_criteria = scopes.criteria.contains("WRITE_OK:")\n'
    '            || scopes.criteria.contains("PATCH_FUNCTION_OK:")\n'
    '            || scopes.criteria.contains("DIFF_PATCH_OK:")\n'
    '            || scopes.criteria.contains("REPLACE_LINES_OK:");',
    1,
)

# ---- 5. Call site: pass scopes.criteria + full inference_text -----------
expect_one(
    "        if let Some(contract) = declared_criteria_contract(node_id, objective, read_only) {\n"
    "            return contract;\n"
    "        }",
    "call-site"
)
src = src.replace(
    "        if let Some(contract) = declared_criteria_contract(node_id, objective, read_only) {\n"
    "            return contract;\n"
    "        }",
    "        if let Some(contract) =\n"
    "            declared_criteria_contract(node_id, scopes.criteria, objective, read_only)\n"
    "        {\n"
    "            return contract;\n"
    "        }",
    1,
)

# ---- 6. PATCH_FUNCTION_OK intent-guard: scope marker-existence check ----
expect_one(
    '        if (objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || (classify_text.contains("patch_function")\n'
    '                && !classify_text.contains("read_function")))\n'
    '            && !read_only',
    "patch-intent"
)
src = src.replace(
    '        if (objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || (classify_text.contains("patch_function")\n'
    '                && !classify_text.contains("read_function")))\n'
    '            && !read_only',
    '        if (scopes.criteria.contains("PATCH_FUNCTION_OK:")\n'
    '            || (classify_text.contains("patch_function")\n'
    '                && !classify_text.contains("read_function")))\n'
    '            && !read_only',
    1,
)

# ---- 7. READ_FUNCTION_OK inline block: scope to scopes.criteria ---------
expect_one(
    '        if let Some(pos) = objective.find("READ_FUNCTION_OK:") {\n'
    '            let remainder = &objective[pos + "READ_FUNCTION_OK:".len()..];',
    "read-fn-inline"
)
src = src.replace(
    '        if let Some(pos) = objective.find("READ_FUNCTION_OK:") {\n'
    '            let remainder = &objective[pos + "READ_FUNCTION_OK:".len()..];',
    '        if let Some(pos) = scopes.criteria.find("READ_FUNCTION_OK:") {\n'
    '            let remainder = &scopes.criteria[pos + "READ_FUNCTION_OK:".len()..];',
    1,
)

# ---- 8. ANALYSIS_OK early-return: scope to scopes.criteria --------------
expect_one(
    '        if objective.contains("ANALYSIS_OK")\n'
    '            && !objective.contains("READ_OK:")\n'
    '            && !objective.contains("SEARCH_CODE_OK:")\n'
    '            && !objective.contains("SEARCH_CHECKED_OK:")\n'
    '        {',
    "analysis-early"
)
src = src.replace(
    '        if objective.contains("ANALYSIS_OK")\n'
    '            && !objective.contains("READ_OK:")\n'
    '            && !objective.contains("SEARCH_CODE_OK:")\n'
    '            && !objective.contains("SEARCH_CHECKED_OK:")\n'
    '        {',
    '        if scopes.criteria.contains("ANALYSIS_OK")\n'
    '            && !scopes.criteria.contains("READ_OK:")\n'
    '            && !scopes.criteria.contains("SEARCH_CODE_OK:")\n'
    '            && !scopes.criteria.contains("SEARCH_CHECKED_OK:")\n'
    '        {',
    1,
)

# ---- 9. has_explicit_path_bound guard: scope to scopes.criteria ---------
expect_one(
    '        let has_explicit_path_bound = objective.contains("READ_OK:")\n'
    '            || objective.contains("WRITE_OK:")\n'
    '            || objective.contains("SEARCH_CODE_OK:")\n'
    '            || objective.contains("SEARCH_CHECKED_OK:")\n'
    '            || objective.contains("SEARCH_MEMORY_OK:")\n'
    '            || objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || objective.contains("READ_FUNCTION_OK:");',
    "path-bound-guard"
)
src = src.replace(
    '        let has_explicit_path_bound = objective.contains("READ_OK:")\n'
    '            || objective.contains("WRITE_OK:")\n'
    '            || objective.contains("SEARCH_CODE_OK:")\n'
    '            || objective.contains("SEARCH_CHECKED_OK:")\n'
    '            || objective.contains("SEARCH_MEMORY_OK:")\n'
    '            || objective.contains("PATCH_FUNCTION_OK:")\n'
    '            || objective.contains("READ_FUNCTION_OK:");',
    '        let has_explicit_path_bound = scopes.criteria.contains("READ_OK:")\n'
    '            || scopes.criteria.contains("WRITE_OK:")\n'
    '            || scopes.criteria.contains("SEARCH_CODE_OK:")\n'
    '            || scopes.criteria.contains("SEARCH_CHECKED_OK:")\n'
    '            || scopes.criteria.contains("SEARCH_MEMORY_OK:")\n'
    '            || scopes.criteria.contains("PATCH_FUNCTION_OK:")\n'
    '            || scopes.criteria.contains("READ_FUNCTION_OK:");',
    1,
)

# ---- 10. Downstream target extraction: scope to scopes.criteria ---------
expect_one(
    '        let read_targets = extract_all_named_targets(objective, "READ_OK:");\n'
    '        let write_targets = extract_all_named_targets(objective, "WRITE_OK:");\n'
    '        let search_code_terms = extract_all_named_targets(objective, "SEARCH_CODE_OK:");\n'
    '        let search_checked_targets = extract_all_named_targets(objective, "SEARCH_CHECKED_OK:");\n'
    '        let search_memory_terms = extract_all_named_targets(objective, "SEARCH_MEMORY_OK:");',
    "target-extraction"
)
src = src.replace(
    '        let read_targets = extract_all_named_targets(objective, "READ_OK:");\n'
    '        let write_targets = extract_all_named_targets(objective, "WRITE_OK:");\n'
    '        let search_code_terms = extract_all_named_targets(objective, "SEARCH_CODE_OK:");\n'
    '        let search_checked_targets = extract_all_named_targets(objective, "SEARCH_CHECKED_OK:");\n'
    '        let search_memory_terms = extract_all_named_targets(objective, "SEARCH_MEMORY_OK:");',
    '        let read_targets = extract_all_named_targets(scopes.criteria, "READ_OK:");\n'
    '        let write_targets = extract_all_named_targets(scopes.criteria, "WRITE_OK:");\n'
    '        let search_code_terms =\n'
    '            extract_all_named_targets(scopes.criteria, "SEARCH_CODE_OK:");\n'
    '        let search_checked_targets =\n'
    '            extract_all_named_targets(scopes.criteria, "SEARCH_CHECKED_OK:");\n'
    '        let search_memory_terms =\n'
    '            extract_all_named_targets(scopes.criteria, "SEARCH_MEMORY_OK:");',
    1,
)

# ---- 11. Final ANALYSIS_OK -> HaltDelivered guard: scope to criteria ----
expect_one(
    '        if objective.contains("ANALYSIS_OK")\n'
    '            && !conditions.iter().any(|c| matches!(c, CompletionCondition::HaltDelivered))\n'
    '        {',
    "analysis-late"
)
src = src.replace(
    '        if objective.contains("ANALYSIS_OK")\n'
    '            && !conditions.iter().any(|c| matches!(c, CompletionCondition::HaltDelivered))\n'
    '        {',
    '        if scopes.criteria.contains("ANALYSIS_OK")\n'
    '            && !conditions.iter().any(|c| matches!(c, CompletionCondition::HaltDelivered))\n'
    '        {',
    1,
)

# ---- 12. Append regression test ----------------------------------------
test_block = '''
    #[test]
    fn objective_prose_mentioning_marker_does_not_mint_condition() {
        // Regression for Mission D failure mode (attempts 1 and 2):
        // a mission whose objective *discusses* a marker (audits, reports,
        // test fixtures) must not inherit that marker as a declared
        // completion condition. Marker mining is authoritative only in the
        // MANDATORY NODE SUCCESS CRITERIA block.
        let mission = concat!(
            "OBJECTIVE: Audit how the executor uses the CMD_OK marker in its evidence stream.\\n",
            "ASSIGNED ROLE: CodeAnalyzer\\n\\n",
            "MANDATORY NODE SUCCESS CRITERIA:\\n",
            "READ_OK:crates/runtime_core/src/executor.rs\\n\\n",
            "NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced.\\n",
        );
        let contract = SubgoalCompletionEngine::infer_contract("t", mission);

        // Objective prose mentions CMD_OK. Pre-fix, declared_criteria_contract
        // was called with the full inference_text, so this mention became a
        // CommandMarkerAcquired condition the agent could not satisfy by
        // reading files.
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::CommandMarkerAcquired
                    | CompletionCondition::CommandExecutionVerified { .. }
            )),
            "objective prose mentioning CMD_OK must not mint a command condition: {:?}",
            contract.conditions
        );

        // The declared criterion is still honored.
        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::ReadPathAcquired { path }
                    if path == "crates/runtime_core/src/executor.rs"
            )),
            "declared READ_OK criterion must survive: {:?}",
            contract.conditions
        );
    }

    #[test]
    fn single_line_directive_without_header_still_mines_markers() {
        // Guard rail: the scoping fix must not break the fallback path.
        // Missions with no MANDATORY NODE SUCCESS CRITERIA header parse
        // with scopes.criteria == raw (from MissionScopes::parse), so a
        // single-line directive keeps working.
        let contract = SubgoalCompletionEngine::infer_contract("t", "READ_OK:crates/foo.rs");
        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::ReadPathAcquired { path }
                    if path == "crates/foo.rs"
            )),
            "single-line directives must still mine markers: {:?}",
            contract.conditions
        );
    }
'''

# Insert before the final closing brace of the tests mod
idx = src.rstrip().rfind('}')
if idx < 0:
    sys.stderr.write("ERROR: no closing brace found\n")
    sys.exit(3)
src = src.rstrip()[:idx].rstrip() + '\n' + test_block + '\n}\n'

open(out_path, 'w', encoding='utf-8').write(src)
PY

echo
echo "===== diff: completion_engine.rs ====="
diff -u "$BACKUP" "$PATCHED" || true
echo "======================================"
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCHED" "$CE"
echo "installed patched completion_engine.rs"

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$BACKUP" "$CE"
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core || rollback

echo ">> cargo test -p nexus_runtime_core --lib completion_engine ..."
cargo test -p nexus_runtime_core --lib completion_engine || rollback

echo ">> cargo test -p nexus_runtime_core --lib 'orchestrator_v2::planner' ..."
cargo test -p nexus_runtime_core --lib 'orchestrator_v2::planner' || rollback

trap - ERR
echo
echo "OK: patch applied and verified."
echo "Next: cargo fmt -p nexus_runtime_core"
echo "Then: cargo test --all-targets --all-features"
