#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — SCOUT MODELS (FULL PAGINATION)
# =============================================================================
set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RESET='\033[0m'

echo -e "${CYAN}>> [SCOUT] Initiating API Model Uplink...${RESET}"

if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

if [ -z "${GEMINI_API_KEY:-}" ]; then
    echo -e "\x1b[31m!! [FAULT] GEMINI_API_KEY is not set.\x1b[0m"
    exit 1
fi

PAGE_TOKEN=""
URL="https://generativelanguage.googleapis.com/v1beta/models?key=${GEMINI_API_KEY}"

echo -e "${YELLOW}>> Fetching active model registry from Google API...${RESET}\n"
echo -e "${GREEN}=== VERIFIED API MODELS ===${RESET}"

while :; do
    if [ -n "$PAGE_TOKEN" ]; then
        REQ_URL="${URL}&pageToken=${PAGE_TOKEN}"
    else
        REQ_URL="${URL}"
    fi

    RESPONSE=$(curl -s "$REQ_URL")
    
    if echo "$RESPONSE" | jq -e '.error' > /dev/null; then
        ERR_MSG=$(echo "$RESPONSE" | jq -r '.error.message')
        echo -e "\x1b[31m!! [API REJECTION] ${ERR_MSG}\x1b[0m"
        exit 1
    fi

    echo "$RESPONSE" | jq -r '
        .models[]? | 
        "\u001b[36m👉 \(.name)\u001b[0m (\(.displayName // "N/A"))\n   \u001b[33mTokens: IN: \(.inputTokenLimit // "N/A") | OUT: \(.outputTokenLimit // "N/A")\u001b[0m\n   Desc: \(.description // "No description provided." | gsub("\n"; " "))\n"
    '
    
    # Extract the next page token. If it doesn't exist, jq returns empty.
    PAGE_TOKEN=$(echo "$RESPONSE" | jq -r '.nextPageToken // empty')
    
    if [ -z "$PAGE_TOKEN" ]; then
        break
    fi
done

echo -e "${CYAN}>> [SCOUT] Network scan complete.${RESET}"
