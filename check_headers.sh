#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [TESTING KERNEL RESPONSE HEADERS]"
# Extract the first JS file from index.html
JS_FILE=$(grep -oP '_app/immutable/entry/start\.\w+\.js' ui/dist/index.html | head -n 1)

echo "Targeting: http://localhost:3000/$JS_FILE"
curl -I http://localhost:3000/$JS_FILE

echo -e "\n>>> [VERIFYING ROOT ELEMENT]"
grep "id=\"svelte\"" ui/dist/index.html || grep "id=\"app\"" ui/dist/index.html || echo "!!! WARNING: No mount point found in index.html"
