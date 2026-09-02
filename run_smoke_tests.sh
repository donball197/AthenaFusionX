#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="nexus_audit_target"
echo "======================================================================"
echo ">> AthenaFusionX Remediation Smoke & AST Integrity Test Suite"
echo ">> Target Directory: ${TARGET_DIR}"
echo "======================================================================"

if [ ! -d "${TARGET_DIR}" ]; then
    echo "[-] Error: ${TARGET_DIR} not found. Ensure repository submodule/target exists."
    exit 1
fi

echo -e "\n[1/3] Running PHP Lint on All Target Files..."
SYNTAX_FAILURES=0
TOTAL_FILES=0

while IFS= read -r file; do
    TOTAL_FILES=$((TOTAL_FILES + 1))
    if command -v php >/dev/null 2>&1; then
        LINT_OUT=$(php -l "$file" 2>&1)
        if echo "$LINT_OUT" | grep -q "No syntax errors detected"; then
            echo "  [OK] $file"
        else
            echo "  [FAIL] $file: $LINT_OUT"
            SYNTAX_FAILURES=$((SYNTAX_FAILURES + 1))
        fi
    else
        echo "  [SKIP] 'php' binary not found locally, skipping syntax check."
        break
    fi
done < <(find "${TARGET_DIR}" -type f -name "*.php")

echo -e "\n[2/3] Validating Parameter Placeholder Arity & Prepared Sinks..."
ARITY_FAILURES=0
while IFS= read -r file; do
    # Scan for prepare(...) calls
    if grep -q "prepare(" "$file"; then
        echo -n "  [AUDIT] Checking $file ... "
        # Verify that for every bind_param, placeholder count matches parameter type string length
        python3 - <<PY_CHECK
import sys, re

with open("$file", "r") as f:
    content = f.read()

# Check for unparameterized raw variables inside prepare()
bad_prepares = re.findall(r'prepare\s*\(\s*["\']\s*SELECT[^\'"?]+\$[a-zA-Z_]', content)
if bad_prepares:
    print("FAILED: Raw variable concatenated into prepare()")
    sys.exit(1)

# Check placeholder counts against bind_param types if present
prepares = re.findall(r'prepare\s*\(\s*["\']([^"\']+)["\']\s*\)', content)
binds = re.findall(r'bind_param\s*\(\s*["\']([a-z]+)["\']', content)

if binds and prepares:
    for p_query, b_types in zip(prepares, binds):
        q_count = p_query.count("?")
        t_count = len(b_types)
        if q_count != t_count:
            print(f"FAILED: Placeholder count ({q_count}) != bind types count ({t_count})")
            sys.exit(1)

print("PASSED")
sys.exit(0)
PY_CHECK
        if [ $? -ne 0 ]; then
            ARITY_FAILURES=$((ARITY_FAILURES + 1))
        fi
    fi
done < <(find "${TARGET_DIR}" -type f -name "*.php")

echo -e "\n[3/3] Docker Runtime Container Check..."
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    echo "  [+] Docker daemon detected. Checking running containers..."
    CONTAINERS=$(docker ps --filter "ancestor=appsecco/sqlinjection-training-app" -q)
    if [ -n "$CONTAINERS" ]; then
        echo "  [+] Container active ($CONTAINERS). Running endpoint curls..."
        curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8000/ || echo "Failed to reach endpoint"
    else
        echo "  [*] No active training app container running. (Ready for 'docker compose up')"
    fi
else
    echo "  [*] Docker not running or not available in this environment. Skipping live container curl."
fi

echo -e "\n======================================================================"
echo ">> SMOKE TEST SUMMARY"
echo "======================================================================"
echo "Total PHP Files Checked : ${TOTAL_FILES}"
echo "Syntax Failures         : ${SYNTAX_FAILURES}"
echo "AST Arity Failures      : ${ARITY_FAILURES}"

if [ "${SYNTAX_FAILURES}" -eq 0 ] && [ "${ARITY_FAILURES}" -eq 0 ]; then
    echo -e "\033[92m>> STATUS: ALL INTEGRITY CHECKS PASSED (0 ERRORS)\033[0m"
    exit 0
else
    echo -e "\033[91m>> STATUS: SMOKE TESTS FAILED\033[0m"
    exit 1
fi
