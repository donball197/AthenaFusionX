#!/bin/bash
# Project Nexus Singularity - Sovereign Auditor v1.0
# Logic: Scans for Apache 2.0 Copyright Header in source files.

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}>> [AUDITOR] Initiating Global DNA Verification...${NC}"

# Target extensions for Nexus DNA
FILES=$(find . -type f \( -name "*.rs" -o -name "*.zig" -o -name "*.ts" -o -name "*.tsx" -o -name "*.svelte" \) -not -path "*/node_modules/*" -not -path "*/target/*")

MISSING_COUNT=0

for f in $FILES
do
    if ! grep -q "Copyright 2026 Donald Ball" "$f"; then
        echo -e "${RED}[FRACTURE] Missing Header:${NC} $f"
        ((MISSING_COUNT++))
    fi
done

if [ $MISSING_COUNT -eq 0 ]; then
    echo -e "${GREEN}>> [SUCCESS] Global DNA is synchronized. All strands carry the Sovereign Fingerprint.${NC}"
else
    echo -e "${RED}>> [ALERT] $MISSING_COUNT files are currently anonymous. Imprint required.${NC}"
fi
