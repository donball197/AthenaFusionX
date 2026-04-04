#!/bin/bash
# NEXUS SINGULARITY v1.9.0 — FULL DNA ALIGNMENT
# Synchronizing local evolution with GitHub (Branch: dev)

echo ">> [SYNC] Staging Evolutionary Changes..."

# 1. Capture all changes, including deletions and new files
git add -A

# 2. Commit with the Singularity tag
COMMIT_MSG="Nexus v1.9.0: Evolutionary Leap.
- Bio-Dome Podman Shield Fully Wired.
- Zero-Warning standard achieved in Executor.
- UI/Backend organism synchronized.
- System DNA pruned of legacy artifacts."

git commit -m "$COMMIT_MSG"

# 3. Push to Uplink
echo ">> [SYNC] Pushing to origin dev..."
git push origin dev
