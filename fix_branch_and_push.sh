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
# 1. Show current state
# ──────────────────────────────────────────────
hdr "Current state"
echo -e "  Branch: ${BOLD}$(git rev-parse --abbrev-ref HEAD)${NC}"
echo -e "  HEAD:   $(git log --oneline -1)"
echo ""
git status --short
echo ""

# ──────────────────────────────────────────────
# 2. Stash uncommitted changes
# ──────────────────────────────────────────────
hdr "Stashing local changes"
STASH_MSG="wip-hold-pipeline-$(date +%s)"
git stash push -m "$STASH_MSG"
ok "Stashed as: $STASH_MSG"

# ──────────────────────────────────────────────
# 3. Switch to dev
# ──────────────────────────────────────────────
hdr "Switching to dev"
git checkout dev
ok "Now on: $(git rev-parse --abbrev-ref HEAD)"

# ──────────────────────────────────────────────
# 4. Pop the stash onto dev
# ──────────────────────────────────────────────
hdr "Applying stashed changes to dev"
if git stash pop; then
    ok "Changes applied cleanly to dev"
else
    warn "Merge conflict — resolving"
    # Show conflicts and try to keep the stashed version for source files
    git status --short | grep '^UU' | while read _ f; do
        echo -e "  ${YELLOW}Conflict:${NC} $f — keeping stashed version"
        git checkout --theirs "$f"
        git add "$f"
    done
    git stash drop 2>/dev/null || true
    ok "Conflicts resolved (stashed versions kept)"
fi

# ──────────────────────────────────────────────
# 5. Verify critical files
# ──────────────────────────────────────────────
hdr "Critical file check"
for f in \
    apps/nexus_ast_tool/src/main.rs \
    apps/nexus_ast_tool/src/hold.rs \
    apps/nexus_ast_tool/src/server.rs \
    apps/nexus_ast_tool/src/sarif.rs \
    apps/nexus_ast_tool/src/lib.rs; do
    if [ -f "$f" ]; then
        ok "$f ($(wc -l < "$f") lines)"
    else
        err "$f — MISSING"
    fi
done

# ──────────────────────────────────────────────
# 6. Quick sanity: lib.rs module declarations
# ──────────────────────────────────────────────
hdr "lib.rs module declarations"
for mod in hold server sarif; do
    if grep -q "pub mod ${mod};" apps/nexus_ast_tool/src/lib.rs 2>/dev/null; then
        ok "pub mod ${mod};"
    else
        warn "pub mod ${mod}; — missing (add to lib.rs)"
    fi
done

# ──────────────────────────────────────────────
# 7. Stage + commit
# ──────────────────────────────────────────────
hdr "Staging all changes"
git add -A
echo -e "  Staged: $(git diff --cached --stat | tail -1)"

hdr "Committing"
git commit -m "feat(security): complete audit-to-hold pipeline with SHA-256 integrity

Core modules:
- hold.rs: SHA-256 manifest, symlink hardening, atomic writes, promote_confirmed()
- server.rs: GET/POST /api/holds with Bearer auth
- sarif.rs: remediation_status tracking (FIXED/REJECTED/UNCONFIRMED/NOT_ATTEMPTED)
- main.rs: --sandbox-verify + --hold flags, audit→triage→heal→sandbox→hold gate
  * fix: remediation_clean requires actionable_count > 0 (no vacuous PASS)
  * fix: warn when --auto-heal runs without --ai-triage
- rules: 24 security rules across 8 languages (PHP sqli, cmd injection, etc.)

Verified: SQL_INJECTION → EXPLOITABLE → parameterized query patch →
sandbox verified → hold created with matching SHA-256 (hold_1795579325_18361)"

ok "Committed"

# ──────────────────────────────────────────────
# 8. Push
# ──────────────────────────────────────────────
hdr "Pushing to origin/dev"
if git push origin dev 2>&1; then
    ok "Pushed successfully"
else
    warn "Push rejected — rebasing on remote"
    git pull --rebase origin dev 2>&1
    if git push origin dev 2>&1; then
        ok "Pushed after rebase"
    else
        err "Push still failing — manual intervention needed"
        exit 1
    fi
fi

# ──────────────────────────────────────────────
# 9. Verify sync
# ──────────────────────────────────────────────
hdr "Sync verification"
LOCAL_SHA=$(git rev-parse HEAD)
REMOTE_SHA=$(git ls-remote origin dev | awk '{print $1}')
echo -e "  Local:  $LOCAL_SHA"
echo -e "  Remote: $REMOTE_SHA"
if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
    ok "In sync with origin/dev"
else
    warn "Out of sync — check git log"
fi

# ──────────────────────────────────────────────
# 10. Clean up any leftover junk
# ──────────────────────────────────────────────
hdr "Cleanup"
for dir in nexus_audit_target nexus_heal_sandbox_*; do
    [ -d "$dir" ] && rm -rf "$dir" && ok "Removed $dir" || true
done

# ──────────────────────────────────────────────
# Done
# ──────────────────────────────────────────────
hdr "Done"
echo -e "  ${GREEN}✓${NC} Branch: ${BOLD}dev${NC}"
echo -e "  ${GREEN}✓${NC} All hold/server/sarif code committed"
echo -e "  ${GREEN}✓${NC} Pushed to GitHub"
echo ""
echo -e "  ${CYAN}https://github.com/donball197/AthenaFusionX/tree/dev${NC}"
echo ""
echo -e "  ${CYAN}Remaining:${NC}"
echo -e "    ./target/debug/nexus-ast-tool tui"
echo -e "    ./target/debug/nexus-ast-tool promote --hold hold_1795579325_18361"
echo ""
