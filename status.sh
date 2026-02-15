#!/bin/bash

# Define Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   NEXUS SINGULARITY // STATUS REPORT   ${NC}"
echo -e "${BLUE}========================================${NC}"

# 1. PROCESS CHECK
if pgrep -f "nexus-terminal" > /dev/null || pgrep -f "nexus_singularity" > /dev/null; then
    echo -e "SYSTEM CORE:  [${GREEN}ONLINE${NC}]"
    
    # 2. API HEALTH CHECK
    # We use a short timeout (2s) so the script doesn't hang if the server is frozen
    RESPONSE=$(curl -s --max-time 2 http://localhost:3000/api/vitals)
    
    if [ -z "$RESPONSE" ]; then
        echo -e "API GATEWAY:  [${RED}UNREACHABLE${NC}]"
    else
        # Extract data safely (works with or without jq)
        if command -v jq &> /dev/null; then
            CPU=$(echo $RESPONSE | jq -r .cpu_usage)
            RAM=$(echo $RESPONSE | jq -r .ram_percent)
            STATUS=$(echo $RESPONSE | jq -r .status)
        else
            # Fallback text parsing if jq is missing
            CPU=$(echo $RESPONSE | grep -o '"cpu_usage":"[^"]*"' | cut -d'"' -f4)
            RAM=$(echo $RESPONSE | grep -o '"ram_percent":"[^"]*"' | cut -d'"' -f4)
            STATUS="CONNECTED"
        fi
        
        echo -e "API GATEWAY:  [${GREEN}CONNECTED${NC}]"
        echo -e " > STATUS:    $STATUS"
        echo -e " > CPU LOAD:  ${CPU}%"
        echo -e " > RAM LOAD:  ${RAM}%"
    fi
else
    echo -e "SYSTEM CORE:  [${RED}OFFLINE${NC}]"
    echo " > Hint: Run ./universal_ignite.sh to start."
fi

echo -e "\n${BLUE}--- EVOLUTION LOG ---${NC}"
if command -v git &> /dev/null; then
    git log -n 3 --pretty=format:"%h - %s (%cr)"
else
    echo "Git not initialized."
fi

echo -e "\n\n${BLUE}--- FORGE STATS ---${NC}"
if [ -f ".forge_stats.json" ]; then
    if command -v jq &> /dev/null; then
        cat .forge_stats.json | jq .
    else
        cat .forge_stats.json
    fi
else
    echo "No forge stats detected."
fi
echo -e "${BLUE}========================================${NC}"
