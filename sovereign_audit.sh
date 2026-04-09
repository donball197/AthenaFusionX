#!/bin/bash
# sovereign_audit.sh - v1.0 [Modular Kernel Baseline]
# Focus: Identify active code bloat while pruning build artifacts and vault history.

THRESHOLD=100
REPORT_DIR="src/reports"
REPORT_FILE="$REPORT_DIR/bloat_audit.txt"

# Ensure report sanctuary exists
mkdir -p "$REPORT_DIR"

echo ">> [AUDITOR] Commencing workspace scan (Threshold: $THRESHOLD lines)..."
echo ">> [AUDITOR] Pruning target/, vault history, and hidden directories..."

# The Core Diagnostic Pipeline:
# 1. Prunes specific high-entropy directories.
# 2. Scans for Rust source strands.
# 3. Filters by line count and formats the output.
find . -type d \( -name "target" -o -name ".git" -o -name ".lazarus_vault" -o -name ".nexus_vault" \) -prune \
    -o -name "*.rs" -type f -exec wc -l {} + 2>/dev/null \
    | awk -v limit="$THRESHOLD" '$1 > limit && $2 != "total" {print $2 ": " $1 " lines"}' \
    | sort -rn -k2 > "$REPORT_FILE"

if [ -s "$REPORT_FILE" ]; then
    echo ">> [AUDITOR] Report forged: $REPORT_FILE"
    echo "--- [Top Bloat Candidates] ---"
    head -n 5 "$REPORT_FILE"
else
    echo ">> [AUDITOR] Nominal. No active strands exceed $THRESHOLD lines."
    rm -f "$REPORT_FILE"
fi
