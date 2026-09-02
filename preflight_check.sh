#!/usr/bin/env bash
set -uo pipefail

# Color Codes
GREEN="\033[1;32m"
RED="\033[1;31m"
YELLOW="\033[1;33m"
CYAN="\033[1;36m"
NC="\033[0m"

echo -e "${CYAN}=================================================="
echo -e ">> ATHENAFUSIONX SOVEREIGN PREFLIGHT DIAGNOSTICS"
echo -e "==================================================${NC}"

if [ ! -f .env ]; then
    echo -e "${RED}[FATAL] .env file not found in current directory!${NC}"
    exit 1
fi

# Export variables from .env
set -a
source .env
set +a

echo -e "Loaded environment configuration from .env\n"

PASS_COUNT=0
TOTAL_COUNT=0

# Helper function to check endpoints
check_endpoint() {
    local name="$1"
    local status="$2"
    local detail="$3"
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
    if [ "$status" -eq 200 ]; then
        echo -e "  [${GREEN}ONLINE${NC}] $name: $detail"
        PASS_COUNT=$((PASS_COUNT + 1))
    elif [ "$status" -eq 401 ] || [ "$status" -eq 403 ]; then
        echo -e "  [${RED}AUTH ERROR${NC}] $name (HTTP $status): Invalid API Key."
    elif [ "$status" -eq 000 ] || [ -z "$status" ]; then
        echo -e "  [${RED}UNREACHABLE${NC}] $name: Could not connect to host ($detail)."
    else
        echo -e "  [${YELLOW}DEGRADED${NC}] $name (HTTP $status): $detail"
    fi
}

# 1. Check Local S23 LiteRT-LM NPU Node
echo -e "${CYAN}--- [1/4] Local S23 Edge Neural Coprocessor ---${NC}"
S23_HOST="${OPENAI_BASE_URL:-http://192.168.4.223:9379/v1}"
S23_STATUS=$(curl -s -o /tmp/s23_models.json -w "%{http_code}" --max-time 3 "${S23_HOST}/models" || echo "000")

if [ "$S23_STATUS" -eq 200 ]; then
    MODEL_ID=$(grep -o '"id": *"[^"]*"' /tmp/s23_models.json | head -n 1 | cut -d'"' -f4 || echo "Active")
    check_endpoint "S23 LiteRT-LM" 200 "Active Model: ${MODEL_ID} @ ${S23_HOST}"
else
    check_endpoint "S23 LiteRT-LM" "$S23_STATUS" "${S23_HOST}"
fi

# 2. Check NVIDIA NIM Cloud API
echo -e "\n${CYAN}--- [2/4] NVIDIA NIM Cloud Gateway ---${NC}"
if [ -n "${NVIDIA_API_KEY:-}" ]; then
    NVIDIA_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 \
        -H "Authorization: Bearer ${NVIDIA_API_KEY}" \
        "https://integrate.api.nvidia.com/v1/models" || echo "000")
    check_endpoint "NVIDIA NIM API" "$NVIDIA_STATUS" "Bearer key validated"
else
    echo -e "  [${YELLOW}SKIPPED${NC}] NVIDIA NIM: NVIDIA_API_KEY is not set in .env"
fi

# 3. Check Google Gemini Cloud API
echo -e "\n${CYAN}--- [3/4] Google Gemini Cloud Gateway ---${NC}"
GEMINI_KEY="${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}"
if [ -n "$GEMINI_KEY" ]; then
    GOOGLE_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 \
        "https://generativelanguage.googleapis.com/v1beta/models?key=${GEMINI_KEY}" || echo "000")
    check_endpoint "Google Gemini API" "$GOOGLE_STATUS" "API key validated"
else
    echo -e "  [${YELLOW}SKIPPED${NC}] Google Gemini: GEMINI_API_KEY is not set in .env"
fi

# 4. Check Bob Agent Endpoint (if configured)
echo -e "\n${CYAN}--- [4/4] Substrate & Mesh State ---${NC}"
if [ -n "${NEXUS_AUTH_KEY:-}" ]; then
    echo -e "  [${GREEN}CONFIGURED${NC}] Security Token: NEXUS_AUTH_KEY present"
fi
if [ -n "${NEXUS_MESH_TOKEN:-}" ]; then
    echo -e "  [${GREEN}CONFIGURED${NC}] Cluster Mesh: NEXUS_MESH_TOKEN present"
fi

echo -e "\n${CYAN}=================================================="
if [ "$PASS_COUNT" -ge 1 ]; then
    echo -e ">> PREFLIGHT STATUS: ${GREEN}SYSTEM READY${NC} (${PASS_COUNT}/${TOTAL_COUNT} Gateways Active)"
else
    echo -e ">> PREFLIGHT STATUS: ${RED}SYSTEM BLOCKED${NC} (0 Active Gateways)"
fi
echo -e "==================================================${NC}"
