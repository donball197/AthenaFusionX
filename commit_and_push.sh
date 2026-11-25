#!/usr/bin/env bash
set -euo pipefail
cd /root/AthenaFusionX

BOLD='\033[1m'
GREEN='\033[32m'
RED='\033[31m'
YELLOW='\033[33m'
CYAN='\033[36m'
NC='\033[0m'

ok()   { echo -e "${GREEN}[✓]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
err()  { echo -e "${RED}[✗]${NC} $1"; }
hdr()  { echo -e "\n${CYAN}${BOLD}=== $1 ===${NC}"; }

# ──────────────────────────────────────────────
# 0. Confirm branch
# ──────────────────────────────────────────────
hdr "Git status"
BRANCH=$(git branch --show-current)
echo -e "  Branch: ${BOLD}${BRANCH}${NC}"
if [ "$BRANCH" != "dev" ]; then
    warn "Expected 'dev' branch but on '${BRANCH}' — switching"
    git checkout dev
fi

REMOTE=$(git remote get-url origin 2>/dev/null || echo "none")
echo -e "  Remote: ${BOLD}${REMOTE}${NC}"

# ──────────────────────────────────────────────
# 1. Show what changed
# ──────────────────────────────────────────────
hdr "Changed files"
git status --short

# ──────────────────────────────────────────────
# 2. Stage the important stuff (exclude temp/clone artifacts)
# ──────────────────────────────────────────────
hdr "Staging changes"

# Exclude the audit clone and sandbox temp dirs
git add apps/nexus_ast_tool/src/main.rs
git add apps/nexus_ast_tool/src/sarif.rs
git add apps/nexus_ast_tool/src/hold.rs
git add apps/nexus_ast_tool/src/server.rs

# Stage any other tracked source changes
git add -u 2>/dev/null || true

# Explicitly unstage junk if it snuck in
git reset -- nexus_audit_target/ 2>/dev/null || true
git reset -- nexus_heal_sandbox_*/ 2>/dev/null || true
git reset -- /tmp/nexus_heal_sandbox_*/ 2>/dev/null || true

hdr "Staged files"
git diff --cached --name-status

# ──────────────────────────────────────────────
# 3. Commit
# ──────────────────────────────────────────────
hdr "Committing"

COMMIT_MSG="fix: audit-to-hold pipeline gap resolved

Root cause: --auto-heal without --ai-triage silently skipped all
findings via the UNTRIAGED fallback path. Triage was never invoked,
so actionable_count=0 and fixed_count=0, and the hold gate at
main.rs:1018 never triggered.

Changes:
- Add warning when --auto-heal runs without --ai-triage
- Fix remediation_clean vacuous PASS (0==0+0 was true)
- Strip debug eprintln lines
- Verified: hold_1795579325_18361 created with SHA-256 integrity"

git commit -m "$COMMIT_MSG"
ok "Committed"

# ──────────────────────────────────────────────
# 4. Push
# ──────────────────────────────────────────────
hdr "Pushing to origin/${BRANCH}"
git push origin "$BRANCH" 2>&1 | tee /tmp/git_push.log

if grep -q "rejected\|error\|fatal" /tmp/git_push.log; then
    err "Push had issues — check output above"
    echo -e "  ${CYAN}If rejected, try:${NC}"
    echo -e "    git pull --rebase origin ${BRANCH}"
    echo -e "    git push origin ${BRANCH}"
else
    ok "Pushed successfully"
fi

# ──────────────────────────────────────────────
# 5. Verify remote state
# ──────────────────────────────────────────────
hdr "Remote verification"
git log --oneline -5
echo ""
git remote -v

# ──────────────────────────────────────────────
# 6. Post-push: run the hold verification one more time
# ──────────────────────────────────────────────
hdr "Hold verification (post-push)"
HOLD="hold_1795579325_18361"
HOLD_DIR=".nexus/holds/${HOLD}"

if [ -d "$HOLD_DIR" ]; then
    ok "Hold still intact: ${HOLD}"

    orig_actual=$(sha256sum "$HOLD_DIR/orig/index.php" | awk '{print $1}')
    orig_manifest=$(python3 -c "
import json
d=json.load(open('$HOLD_DIR/hold.json'))
print(d['files'][0]['orig_sha256'])")

    if [ "$orig_actual" = "$orig_manifest" ]; then
        ok "SHA-256 integrity: PASS"
    else
        err "SHA-256 integrity: FAIL"
    fi
else
    warn "Hold directory not found (was it cleaned up?)"
fi

# ──────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────
hdr "Done"
echo -e "  ${GREEN}✓${NC} Committed to ${BOLD}${BRANCH}${NC}"
echo -e "  ${GREEN}✓${NC} Pushed to ${BOLD}${REMOTE}${NC}"
echo ""
echo -e "  ${CYAN}GitHub:${NC} https://github.com/donball197/AthenaFusionX/tree/${BRANCH}"
echo ""
echo -e "  ${CYAN}Remaining manual steps:${NC}"
echo -e "    1. TUI check:     ${BOLD}./target/debug/nexus-ast-tool tui${NC}"
echo -e "    2. Test promote:  ${BOLD}./target/debug/nexus-ast-tool promote --hold hold_1795579325_18361${NC}"
echo -e "    3. Verify patch:  ${BOLD}cat /tmp/test_heal/index.php${NC} (should show parameterized query)"
echo -e "    4. Clean temp:    ${BOLD}rm -rf nexus_audit_target/ nexus_heal_sandbox_*/${NC}"
echo ""
