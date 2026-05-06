#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [1/2] CHECKING SOURCE TEMPLATE"
if [ -f ui/src/app.html ]; then
    cat ui/src/app.html
else
    echo "!!! ERROR: ui/src/app.html not found!"
fi

echo -e "\n>>> [2/2] INSPECTING DIST BODY"
# Show the body tag of the compiled file
grep -A 2 "<body" ui/dist/index.html
