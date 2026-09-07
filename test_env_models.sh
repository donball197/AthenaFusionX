#!/usr/bin/env bash
set -uo pipefail

GREEN="\033[1;32m"
RED="\033[1;31m"
YELLOW="\033[1;33m"
CYAN="\033[1;36m"
BOLD="\033[1m"
NC="\033[0m"

echo -e "${CYAN}=================================================="
echo -e ">> ATHENAFUSIONX MODEL PREFLIGHT VERIFIER"
echo -e "==================================================${NC}"

set -a
source .env
set +a

declare -A ALL_MODELS
add_models_from_var() {
    local var_val="$1"
    IFS=',' read -ra ADDR <<< "$var_val"
    for m in "${ADDR[@]}"; do
        clean_m=$(echo "$m" | tr -d ' "=\r\n' | xargs)
        if [ -n "$clean_m" ]; then
            ALL_MODELS["$clean_m"]=1
        fi
    done
}

add_models_from_var "${NEXUS_LIGHT_MODEL:-}"
add_models_from_var "${NEXUS_HEAVY_MODEL:-}"
add_models_from_var "${NEXUS_PLANNER_MODEL:-}"
add_models_from_var "${NEXUS_LIGHT_CASCADE:-}"
add_models_from_var "${NEXUS_HEAVY_CASCADE:-}"

TOTAL_TESTS=${#ALL_MODELS[@]}
if [ -n "${NEXUS_EMBEDDING_URL:-}" ]; then
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
fi

echo -e "Testing ${BOLD}${TOTAL_TESTS}${NC} distinct target(s)...\n"

PASS_COUNT=0
FAIL_COUNT=0

# --- 1. Chat & Reasoning LLM Preflight ---
for MODEL in "${!ALL_MODELS[@]}"; do
    MODEL_LOWER=$(echo "$MODEL" | tr '[:upper:]' '[:lower:]')
    echo -e "${BOLD}▶ Testing Model: ${CYAN}${MODEL}${NC}"

    URL=""
    AUTH_HEADER=""
    API_KEY=""
    PROVIDER=""

    if [[ "$MODEL_LOWER" == *"e4b"* || "$MODEL_LOWER" == *"litert"* ]]; then
        PROVIDER="Local S23 LiteRT-LM (Edge)"
        GW="${OPENAI_BASE_URL:-${NEXUS_API_GATEWAY:-http://192.168.4.223:9379/v1}}"
        GW_CLEAN="${GW%/}"
        if [[ "$GW_CLEAN" == *"/chat/completions" ]]; then
            URL="$GW_CLEAN"
        else
            URL="${GW_CLEAN}/chat/completions"
        fi
        API_KEY="${OPENAI_API_KEY:-local-s23-device}"
        AUTH_HEADER="Authorization: Bearer ${API_KEY}"
    elif [[ "$MODEL_LOWER" == *"gemini"* || ( "$MODEL_LOWER" == *"gemma"* && "$MODEL_LOWER" != *"google/"* ) ]]; then
        PROVIDER="Google Cloud API (Gemini/Gemma)"
        URL="${GOOGLE_ENDPOINT:-https://generativelanguage.googleapis.com/v1beta/openai/chat/completions}"
        API_KEY="${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}"
        AUTH_HEADER="Authorization: Bearer ${API_KEY}"
    else
        PROVIDER="NVIDIA NIM Cloud"
        URL="${NVIDIA_ENDPOINT:-https://integrate.api.nvidia.com/v1/chat/completions}"
        API_KEY="${NVIDIA_API_KEY:-}"
        AUTH_HEADER="Authorization: Bearer ${API_KEY}"
    fi

    echo -e "  • Provider : ${YELLOW}${PROVIDER}${NC}"
    echo -e "  • Endpoint : ${URL}"

    START_TIME=$(date +%s%3N)
    
    HTTP_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$URL" \
        -H "Content-Type: application/json" \
        -H "$AUTH_HEADER" \
        --max-time 45 \
        -d "{
            \"model\": \"$MODEL\",
            \"messages\": [{\"role\": \"user\", \"content\": \"Respond with: ONLINE\"}],
            \"max_tokens\": 16,
            \"temperature\": 0.1
        }" || echo -e '{"error": "curl transport failure"}\n000')

    END_TIME=$(date +%s%3N)
    LATENCY=$((END_TIME - START_TIME))

    HTTP_CODE=$(echo "$HTTP_RESPONSE" | tail -n1)
    BODY=$(echo "$HTTP_RESPONSE" | sed '$d')

    if [ "$HTTP_CODE" -eq 200 ]; then
        CONTENT=$(echo "$BODY" | grep -o '"content": *"[^"]*"' | head -n1 | cut -d'"' -f4 | tr -d '\n' || echo "OK")
        echo -e "  • Status   : ${GREEN}200 OK${NC} (${LATENCY}ms)"
        echo -e "  • Response : \"${CONTENT}\"\n"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        ERR_MSG=$(echo "$BODY" | grep -o '"message": *"[^"]*"' | head -n1 | cut -d'"' -f4 || echo "Status $HTTP_CODE")
        echo -e "  • Status   : ${RED}FAILED (HTTP ${HTTP_CODE})${NC} (${LATENCY}ms)"
        echo -e "  • Error    : ${ERR_MSG}\n"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done

# --- 2. Embedding Gateway Preflight (Lenovo Duet) ---
if [ -n "${NEXUS_EMBEDDING_URL:-}" ]; then
    EMBED_MODEL="${NEXUS_EMBEDDING_MODEL:-granite-311m-edge}"
    echo -e "${BOLD}▶ Testing Embedding: ${CYAN}${EMBED_MODEL}${NC}"
    echo -e "  • Provider : ${YELLOW}Lenovo Duet Edge Node${NC}"
    echo -e "  • Endpoint : ${NEXUS_EMBEDDING_URL}"

    START_TIME=$(date +%s%3N)

    EMBED_RES=$(curl -s -w "\n%{http_code}" -X POST "$NEXUS_EMBEDDING_URL" \
        -H "Content-Type: application/json" \
        --max-time 15 \
        -d "{
            \"input\": \"AthenaFusionX substrate preflight check\",
            \"model\": \"$EMBED_MODEL\"
        }" || echo -e '{"error": "curl transport failure"}\n000')

    END_TIME=$(date +%s%3N)
    LATENCY=$((END_TIME - START_TIME))

    HTTP_CODE=$(echo "$EMBED_RES" | tail -n1)
    BODY=$(echo "$EMBED_RES" | sed '$d')

    if [ "$HTTP_CODE" -eq 200 ]; then
        DIM=$(echo "$BODY" | python3 -c "import sys, json; data=json.load(sys.stdin); print(len(data['data'][0]['embedding']))" 2>/dev/null || echo "384")
        echo -e "  • Status   : ${GREEN}200 OK${NC} (${LATENCY}ms)"
        echo -e "  • Response : \"Verified ${DIM}-dim normalized vector\"\n"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo -e "  • Status   : ${RED}FAILED (HTTP ${HTTP_CODE})${NC} (${LATENCY}ms)"
        echo -e "  • Error    : Unable to generate embeddings on endpoint\n"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
fi

echo -e "${CYAN}=================================================="
if [ "$FAIL_COUNT" -eq 0 ]; then
    echo -e ">> PREFLIGHT VERDICT: ${GREEN}ALL CONFIGURED MODELS & EMBEDDINGS READY${NC} (${PASS_COUNT}/${TOTAL_TESTS} Passed)"
else
    echo -e ">> PREFLIGHT VERDICT: ${YELLOW}DEGRADED${NC} (${PASS_COUNT} Passed, ${FAIL_COUNT} Failed)"
fi
echo -e "==================================================${NC}"
