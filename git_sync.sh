#!/bin/bash
# NEXUS SINGULARITY v1.9.0 — DEV BRANCH SYNC (Hardened)
# Purpose: Ensure DNA is physically pushed before reporting success.

echo ">> [SYNC] Preparing Nexus DNA for Uplink..."

# 1. Credentials Configuration (Run this once to cache your token)
git config --global credential.helper store

# 2. Stage core logic and glass
git add .gitignore src/ ui/src/ ui/*.js ui/*.json Cargo.toml Cargo.lock audits/ *.sh
git add .lazarus/ledger.json 2>/dev/null

# 3. Commit (Allowing empty if no changes since last failed attempt)
COMMIT_MSG="Nexus v1.9.0: Singularity Baseline (Retry Auth)"
git commit -m "$COMMIT_MSG" || echo ">> [SYNC] No new local changes to commit."

# 4. Push with Verification
echo ">> [SYNC] Pushing DNA to GitHub (Branch: dev)..."
if git push origin dev; then
    echo ">> [SYNC] Evolution archived. Shared DNA is now live on origin/dev."
else
    echo "!! [SYNC] UPLINK FAILED. Check your GitHub Token."
    exit 1
fi
