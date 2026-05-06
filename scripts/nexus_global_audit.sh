#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1

echo -e "\x1b[35m>> [NEXUS] INITIATING GLOBAL HIERARCHICAL AUDIT (RATE-LIMITED)\x1b[0m"

mkdir -p .nexus_ast
rm -f .nexus_ast/*_summary.jsonl

# Define the logical domains
DOMAINS=(
    "ui:ui/src"
    "kernel:src/kernel"
    "cortex:src/neural_agent.rs src/task_manager.rs src/orchestrator.rs"
    "infrastructure:src/server.rs src/executor.rs src/lazarus_vault.rs"
)

# 1. THE MAP PHASE: Audit each domain individually
for DOMAIN_DATA in "${DOMAINS[@]}"; do
    NAME="${DOMAIN_DATA%%:*}"
    PATHS="${DOMAIN_DATA##*:}"
    
    echo -e "\n\x1b[36m[*] Mapping Domain: $NAME\x1b[0m"
    PAYLOAD=".nexus_ast/${NAME}_payload.jsonl"
    rm -f "$PAYLOAD"
    touch "$PAYLOAD"
    
    # Ingest Svelte, JS, TS, and Rust files for the specific domain
    for PATH_ITEM in $PATHS; do
        if [ -d "$PATH_ITEM" ]; then
            find "$PATH_ITEM" -type f \( -name "*.rs" -o -name "*.svelte" -o -name "*.js" \) -exec cat {} + >> "$PAYLOAD"
        elif [ -f "$PATH_ITEM" ]; then
            cat "$PATH_ITEM" >> "$PAYLOAD"
        fi
    done
    
    echo "    -> Compiling $NAME sector..."
    cargo run --release --bin auditor "$PAYLOAD" > ".nexus_ast/${NAME}_summary.txt"
    
    echo -e "\x1b[90m    [+] Cooling down API token window (15s)...\x1b[0m"
    sleep 15
done

# 2. THE REDUCE PHASE: The Grand Synthesis
echo -e "\n\x1b[35m[*] Initiating Grand Synthesis...\x1b[0m"
SYNTHESIS_PAYLOAD=".nexus_ast/global_synthesis.jsonl"
rm -f "$SYNTHESIS_PAYLOAD"

echo "=== UI DOMAIN SUMMARY ===" >> "$SYNTHESIS_PAYLOAD"
cat .nexus_ast/ui_summary.txt >> "$SYNTHESIS_PAYLOAD"
echo -e "\n=== KERNEL DOMAIN SUMMARY ===" >> "$SYNTHESIS_PAYLOAD"
cat .nexus_ast/kernel_summary.txt >> "$SYNTHESIS_PAYLOAD"
echo -e "\n=== CORTEX DOMAIN SUMMARY ===" >> "$SYNTHESIS_PAYLOAD"
cat .nexus_ast/cortex_summary.txt >> "$SYNTHESIS_PAYLOAD"
echo -e "\n=== INFRASTRUCTURE DOMAIN SUMMARY ===" >> "$SYNTHESIS_PAYLOAD"
cat .nexus_ast/infrastructure_summary.txt >> "$SYNTHESIS_PAYLOAD"

echo "    -> Synthesizing Global Architecture..."
cargo run --release --bin auditor "$SYNTHESIS_PAYLOAD"

echo -e "\x1b[32m>> [GLOBAL AUDIT COMPLETE]\x1b[0m"
