#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [CHECKING UI MANIFEST]"
grep -oP 'src="/_app/immutable/entry/start\.\w+\.js"' ui/dist/index.html
echo ">>> [VERIFYING FILE EXISTENCE]"
ls -l ui/dist/_app/immutable/entry/start.*.js
echo ">>> [CHECKING FOR EMPTY ASSETS]"
find ui/dist -type f -empty
