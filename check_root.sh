#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [CHECKING MODULE DECLARATIONS]"
for mod in client monitor agent orchestrator memory arbiter mission_control ast_route server_terminal scheduler; do
    grep -q "pub mod $mod;" src/main.rs || grep -q "pub mod $mod;" src/lib.rs || echo "!!! MISSING: pub mod $mod;"
done
