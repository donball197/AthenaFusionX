#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
git switch -c audit-hardening 2>/dev/null || git switch audit-hardening
for p in 'audit_out/' '*.prepatch' '.nexus_daemon.lock'; do
  grep -qxF "$p" .gitignore || echo "$p" >> .gitignore
done
git add -A
if git diff --cached --name-only | grep -E '(^|/)\.env|\.db|\.bak|\.prepatch|ui_token|\.lock$'; then
  echo "!! unwanted file staged, aborting"; git reset -q; exit 1
fi
git commit -q -m "WIP snapshot: orchestrator hardening (earlier work) + ide_backend audit patches"
git log --oneline -1
git diff --cached --stat >/dev/null; git status --short | wc -l
