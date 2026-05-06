#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1

TARGET="$1"
PAYLOAD="/tmp/audit_payload.jsonl"

if [ -z "$TARGET" ]; then
    echo -e "\x1b[31m!! [FAULT] No target specified. Usage: ./nexus_audit.sh <file_or_directory>\x1b[0m"
    exit 1
fi

echo -e "\x1b[36m[*] Initiating Sovereign Audit at: $TARGET\x1b[0m"
echo "[*] Compressing fragments..."

# 1. Purge the ghost cache
rm -f "$PAYLOAD"
touch "$PAYLOAD"

# 2. Universal Ingestion (File or Directory)
if [ -d "$TARGET" ]; then
    # Recursively grab Rust, Svelte, JS, and TS files
    find "$TARGET" -type f \( -name "*.rs" -o -name "*.svelte" -o -name "*.js" -o -name "*.ts" \) -exec cat {} + >> "$PAYLOAD"
elif [ -f "$TARGET" ]; then
    cat "$TARGET" >> "$PAYLOAD"
else
    echo -e "\x1b[31m!! [FAULT] Target '$TARGET' not found.\x1b[0m"
    exit 1
fi

# 3. Telemetry Calculation
BYTES=$(wc -c < "$PAYLOAD")
if [ "$BYTES" -eq 0 ]; then
    echo -e "\x1b[33m!! [WARN] Zero bytes ingested. Check target path.\x1b[0m"
    exit 1
fi

TOKENS=$((BYTES / 4))
echo "[*] Estimated Tokens: $TOKENS / 15000 (Limit)"

# 4. Ignite the Rust Auditor
cargo run --release --bin auditor "$PAYLOAD"
