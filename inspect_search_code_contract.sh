#!/usr/bin/env bash
set -euo pipefail

ROOT="${HOME}/AthenaFusionx"
cd "$ROOT"

echo "=================================================="
echo "NEXUS SEARCH_CODE CONTRACT — READ-ONLY AUDIT"
echo "=================================================="
echo "ROOT: $ROOT"
echo

echo "=================================================="
echo "1. ACTION MATCHER"
echo "=================================================="
rg -n -C 12 \
  'fn action_matches_strategy|search_code:|expected_query|query\.trim\(\).*expected_query' \
  crates/runtime_core/src/cortical_loop.rs || true

echo
echo "=================================================="
echo "2. ACTION MATCHER CALL SITES"
echo "=================================================="
rg -n -C 8 \
  'action_matches_strategy\(' \
  crates/runtime_core/src/cortical_loop.rs || true

echo
echo "=================================================="
echo "3. ACTIVE STRATEGY VS h_content"
echo "=================================================="
rg -n -C 8 \
  'active_strategy|h_content|action_matches_strategy' \
  crates/runtime_core/src/cortical_loop.rs | head -n 260 || true

echo
echo "=================================================="
echo "4. SEARCH_CODE VERIFIER"
echo "=================================================="
rg -n -C 12 \
  'SEARCH_CODE_OK:|SearchMarkerAcquired|evaluate_contract_criteria' \
  crates/runtime_core/src/orchestrator_v2/verifier.rs \
  crates/runtime_core/src/outcome_verifier.rs || true

echo
echo "=================================================="
echo "5. SEARCH_CODE EXECUTION EVIDENCE"
echo "=================================================="
rg -n -C 14 \
  'MotorAction::SearchCode|\[SEARCH_CODE_OK\]|Query:' \
  crates/runtime_core/src/executor.rs \
  crates/runtime_core/src/evidence_markers.rs || true

echo
echo "=================================================="
echo "6. EPISTEMIC SEARCH EVIDENCE PARSER"
echo "=================================================="
rg -n -C 12 \
  '\[SEARCH_CODE_OK\]|split\("Query:"\)|split\("Path:"\)' \
  crates/runtime_core/src/cognition/epistemic_session.rs || true

echo
echo "=================================================="
echo "7. SEARCH_CODE ACTION PROTOCOL"
echo "=================================================="
rg -n -C 8 \
  'SearchCode|search_code' \
  crates/runtime_core/src/agent_protocol.rs \
  crates/runtime_core/src/tools/schema.rs \
  crates/runtime_core/src/action_intercept.rs || true

echo
echo "=================================================="
echo "8. DETERMINISTIC SEARCH_CODE STRATEGY BINDING"
echo "=================================================="
rg -n -C 14 \
  'SEARCH_CODE_OK:|search_code:crates/runtime_core/src' \
  crates/runtime_core/src/epistemic_engine.rs || true

echo
echo "=================================================="
echo "9. EXISTING SEARCH_CODE TESTS"
echo "=================================================="
rg -n -C 10 \
  'search_code|SearchCode|SEARCH_CODE_OK|action_matches_strategy' \
  crates/runtime_core/src \
  --glob '*.rs' \
  | head -n 320 || true

echo
echo "=================================================="
echo "10. QUERY-FIDELITY PATTERNS"
echo "=================================================="
rg -n \
  'contains\(.*query|query.*contains|expected_query|actual.*query|Query:' \
  crates/runtime_core/src \
  --glob '*.rs' \
  | head -n 260 || true

echo
echo "=================================================="
echo "11. GIT STATUS — READ ONLY"
echo "=================================================="
git status --short

echo
echo "=================================================="
echo "AUDIT COMPLETE — NO SOURCE FILES MODIFIED"
echo "=================================================="
