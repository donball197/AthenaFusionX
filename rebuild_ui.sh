#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [1/4] NUCLEAR CLEANING"
rm -rf ui/dist ui/.svelte-kit ui/node_modules/.vite
mkdir -p ui/dist

echo ">>> [2/4] CHECKING ENVIRONMENT"
df -h . | grep -v Filesystem  # Ensure we aren't at 100% disk usage

echo ">>> [3/4] VERBOSE UI BUILD"
cd ui && npm run build -- --debug

echo ">>> [4/4] POST-BUILD INTEGRITY CHECK"
cd ..
ls -lh ui/dist/index.html
ls -lh ui/dist/_app/immutable/entry/start.*.js
