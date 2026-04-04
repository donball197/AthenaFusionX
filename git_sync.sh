#!/bin/bash
# NEXUS SINGULARITY v1.9.0 — DEV BRANCH SYNC
# Purpose: Synchronize evolved DNA to the 'dev' branch.

echo ">> [SYNC] Aligning DNA with 'dev' branch..."

# 1. Stage core logic and glass
git add .gitignore
git add src/
git add ui/src/
git add ui/*.js ui/*.json
git add Cargo.toml Cargo.lock
git add *.sh  # Catching the universal ignition scripts

# 2. Stage persistent intelligence (Audits and Ledger)
git add audits/
git add .lazarus/ledger.json 2>/dev/null

# 3. Commit
COMMIT_MSG="Nexus v1.9.0: Singularity Baseline. 
- Recursive Vision active (Rust/Zig/Svelte/C++).
- Podman Bio-Dome operationalized in Executor.
- SQLite Memory Palace persistent.
- Branch: dev."

git commit -m "$COMMIT_MSG"

# 4. Push to DEV
echo ">> [SYNC] Pushing DNA to GitHub (Branch: dev)..."
git push origin dev

echo ">> [SYNC] Evolution archived. Shared DNA is now live on origin/dev."
