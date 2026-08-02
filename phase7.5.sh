#!/usr/bin/env bash
set -euo pipefail

echo "======================================================="
echo " AthenaFusionX Phase 7.5"
echo " Execution Reliability Validation"
echo "======================================================="

REPORT_DIR="repository_analysis"
EVIDENCE_DIR="${REPORT_DIR}/evidence"

mkdir -p "$EVIDENCE_DIR"

echo
echo "[1/10] Formatting workspace..."
cargo fmt --all

echo
echo "[2/10] Clippy validation..."
cargo clippy --workspace --all-targets -- -D warnings

echo
echo "[3/10] Building runtime..."
cargo build --workspace

echo
echo "[4/10] Building cognition tools..."

cargo build --bin repository_planner
cargo build --bin symbol_graph

if cargo metadata --format-version=1 | grep -q '"name":"impact_analyzer"'; then
    cargo build --bin impact_analyzer
else
    echo "Impact analyzer not yet present (expected)"
fi

echo
echo "[5/10] Running repository planner..."
cargo run --bin repository_planner

echo
echo "[6/10] Running symbol graph..."
cargo run --bin symbol_graph

echo
echo "[7/10] Validating generated intelligence..."

for FILE in \
    repository_analysis/mutation_map.json \
    repository_analysis/symbol_graph.json
do
    if [[ ! -f "$FILE" ]]; then
        echo "Missing artifact: $FILE"
        exit 1
    fi

    jq empty "$FILE" >/dev/null

done

echo "✓ Repository intelligence valid"

echo
echo "[8/10] Searching for repeated execution failures..."

grep -R "PATH_TRAVERSAL_DENIED" .nexus 2>/dev/null \
    | tee "${EVIDENCE_DIR}/path_denials.log" || true

grep -R "Failed to parse AgentAction JSON" .nexus 2>/dev/null \
    | tee "${EVIDENCE_DIR}/translator_failures.log" || true

grep -R "Strategy completely abandoned" .nexus 2>/dev/null \
    | tee "${EVIDENCE_DIR}/abandoned_strategies.log" || true

echo
echo "[9/10] Produce execution metrics..."

cat > "${EVIDENCE_DIR}/execution_validation.json" <<EOF
{
  "phase": 7.5,
  "timestamp": "$(date -Iseconds)",
  "artifacts": {
    "mutation_map": "repository_analysis/mutation_map.json",
    "symbol_graph": "repository_analysis/symbol_graph.json"
  },
  "logs": {
    "path_denials": "repository_analysis/evidence/path_denials.log",
    "translator_failures": "repository_analysis/evidence/translator_failures.log",
    "abandoned_strategies": "repository_analysis/evidence/abandoned_strategies.log"
  }
}
EOF

echo
echo "[10/10] Workspace status"

git status --short

echo
echo "======================================================="
echo " Phase 7.5 COMPLETE"
echo "======================================================="
echo
echo "Next milestone:"
echo
echo "Repository Intelligence"
echo "        │"
echo "        ▼"
echo "Execution Reliability"
echo "        │"
echo "        ▼"
echo "Impact Analysis"
echo "        │"
echo "        ▼"
echo "Mutation Ranking"
echo "        │"
echo "        ▼"
echo "Repair Planning"
echo "        │"
echo "        ▼"
echo "Autonomous Refactoring"
