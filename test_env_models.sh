#!/usr/bin/env bash
set -uo pipefail

GREEN="\033[1;32m"
RED="\033[1;31m"
YELLOW="\033[1;33m"
CYAN="\033[1;36m"
BOLD="\033[1m"
DIM="\033[2m"
NC="\033[0m"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo -e "${CYAN}=================================================="
echo -e ">> ATHENAFUSIONX MODEL PREFLIGHT VERIFIER v2"
echo -e "==================================================${NC}"

if [ ! -f .env ]; then
    echo -e "${RED}ERROR: .env not found${NC}"
    exit 1
fi

# Export .env variables to child processes without printing secrets.
set -a
# shellcheck disable=SC1091
source .env
set +a

# --------------------------------------------------
# Configuration
# --------------------------------------------------

NVIDIA_URL="${NVIDIA_ENDPOINT:-https://integrate.api.nvidia.com/v1/chat/completions}"
GOOGLE_URL="${GOOGLE_ENDPOINT:-https://generativelanguage.googleapis.com/v1beta/openai/chat/completions}"
EDGE_GW="${OPENAI_BASE_URL:-${NEXUS_API_GATEWAY:-http://192.168.4.223:9379/v1}}"
EMBED_URL="${NEXUS_EMBEDDING_URL:-http://127.0.0.1:9380/v1/embeddings}"

NVIDIA_TIMEOUT="${NEXUS_NVIDIA_PREFLIGHT_TIMEOUT:-60}"
GOOGLE_TIMEOUT="${NEXUS_GOOGLE_PREFLIGHT_TIMEOUT:-35}"
EDGE_TIMEOUT="${NEXUS_EDGE_PREFLIGHT_TIMEOUT:-60}"
EMBED_TIMEOUT="${NEXUS_EMBED_PREFLIGHT_TIMEOUT:-15}"

RETRIES="${NEXUS_PREFLIGHT_RETRIES:-2}"

# --------------------------------------------------
# Secret sanity checks
# --------------------------------------------------

echo
echo -e "${BOLD}ENVIRONMENT${NC}"

if [ -n "${NVIDIA_API_KEY:-}" ]; then
    echo "  NVIDIA_API_KEY : SET (${#NVIDIA_API_KEY} chars)"
else
    echo -e "  NVIDIA_API_KEY : ${RED}NOT SET${NC}"
fi

if [ -n "${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}" ]; then
    echo "  GOOGLE/GEMINI KEY : SET"
else
    echo -e "  GOOGLE/GEMINI KEY : ${YELLOW}NOT SET${NC}"
fi

echo "  NVIDIA endpoint : ${NVIDIA_URL}"
echo "  Google endpoint : ${GOOGLE_URL}"
echo "  Edge gateway    : ${EDGE_GW}"
echo "  Embed endpoint  : ${EMBED_URL}"

# --------------------------------------------------
# Model collection
# --------------------------------------------------

declare -A ALL_MODELS=()

add_models_from_var() {
    local var_val="${1:-}"
    local m clean_m

    IFS=',' read -ra ADDR <<< "$var_val"

    for m in "${ADDR[@]}"; do
        clean_m="$(printf '%s' "$m" | sed 's/[[:space:]]//g' | sed 's/^"//;s/"$//;s/^'\''//;s/'\''$//')"

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
add_models_from_var "${NEXUS_PLANNER_CASCADE:-}"

TOTAL_MODELS=${#ALL_MODELS[@]}

if [ -n "${NEXUS_EMBEDDING_URL:-}" ]; then
    TOTAL_TESTS=$((TOTAL_MODELS + 1))
else
    TOTAL_TESTS=$TOTAL_MODELS
fi

echo
echo -e "Testing ${BOLD}${TOTAL_TESTS}${NC} configured target(s)..."
echo

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0
SLOW_COUNT=0

# --------------------------------------------------
# Provider selection
# --------------------------------------------------

resolve_provider() {
    local model_lower="$1"

    if [[ "$model_lower" == *"e4b"* ||
          "$model_lower" == *"litert"* ||
          "$model_lower" == "nexus" ]]; then
        echo "edge"
        return
    fi

    if [[ "$model_lower" == *"gemini"* ]]; then
        echo "google"
        return
    fi

    # z-ai GLM is NVIDIA NIM for the current AthenaFusionX setup.
    if [[ "$model_lower" == z-ai/* ||
          "$model_lower" == *"nvidia"* ||
          "$model_lower" == *"nemotron"* ||
          "$model_lower" == *"deepseek"* ||
          "$model_lower" == meta/* ||
          "$model_lower" == google/* ||
          "$model_lower" == *"mistral"* ]]; then
        echo "nvidia"
        return
    fi

    # Bare Gemma is deliberately treated as Google here unless
    # it is the known local E4B edge model handled above.
    if [[ "$model_lower" == *"gemma"* ]]; then
        echo "google"
        return
    fi

    echo "nvidia"
}

provider_name() {
    case "$1" in
        nvidia) echo "NVIDIA NIM Cloud" ;;
        google) echo "Google Cloud API" ;;
        edge)   echo "Local S23 LiteRT-LM (Edge)" ;;
        *)      echo "Unknown" ;;
    esac
}

provider_url() {
    case "$1" in
        nvidia) echo "$NVIDIA_URL" ;;
        google) echo "$GOOGLE_URL" ;;
        edge)
            local clean="${EDGE_GW%/}"
            if [[ "$clean" == */chat/completions ]]; then
                echo "$clean"
            else
                echo "${clean}/chat/completions"
            fi
            ;;
        *) echo "" ;;
    esac
}

provider_key() {
    case "$1" in
        nvidia) echo "${NVIDIA_API_KEY:-}" ;;
        google) echo "${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}" ;;
        edge)   echo "${OPENAI_API_KEY:-local-s23-device}" ;;
        *)      echo "" ;;
    esac
}

provider_timeout() {
    case "$1" in
        nvidia) echo "$NVIDIA_TIMEOUT" ;;
        google) echo "$GOOGLE_TIMEOUT" ;;
        edge)   echo "$EDGE_TIMEOUT" ;;
        *)      echo 30 ;;
    esac
}

# --------------------------------------------------
# Single model test
# --------------------------------------------------

test_model() {
    local model="$1"
    local model_lower
    model_lower="$(printf '%s' "$model" | tr '[:upper:]' '[:lower:]')"

    local provider
    provider="$(resolve_provider "$model_lower")"

    local pname
    pname="$(provider_name "$provider")"

    local url
    url="$(provider_url "$provider")"

    local key
    key="$(provider_key "$provider")"

    local timeout
    timeout="$(provider_timeout "$provider")"

    local tmp
    tmp="$(mktemp)"

    local attempt=0
    local http_code="000"
    local curl_rc=0
    local elapsed="0"

    while [ "$attempt" -le "$RETRIES" ]; do
        attempt=$((attempt + 1))

        local start_ms
        local end_ms

        start_ms="$(date +%s%3N)"

        payload="$(python3 - "$provider" "$model" <<'PY_PAYLOAD'
import json
import sys

provider = sys.argv[1]
model = sys.argv[2]

payload = {
    "model": model,
    "messages": [
        {"role": "user", "content": "Reply with exactly ONLINE"}
    ],
    "max_tokens": 16,
    "temperature": 0,
}

if provider == "nvidia" and model.lower().startswith("z-ai/"):
    payload["reasoning_effort"] = "low"
    payload["max_tokens"] = 64
elif provider == "nvidia":
    payload["chat_template_kwargs"] = {
        "enable_thinking": False,
        "thinking": False,
        "force_nonempty_content": True,
    }

print(json.dumps(payload))
PY_PAYLOAD
)"

        curl -sS \
            --connect-timeout 10 \
            --max-time "$timeout" \
            -o "$tmp" \
            -w '%{http_code}' \
            -X POST "$url" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer ${key}" \
            -d "$payload" \
            > "${tmp}.status" 2>/dev/null

        curl_rc=$?
        http_code="$(cat "${tmp}.status" 2>/dev/null || echo 000)"

        end_ms="$(date +%s%3N)"
        elapsed=$((end_ms - start_ms))

        if [ "$curl_rc" -eq 0 ] && [ "$http_code" = "200" ]; then
            break
        fi

        # Retry transport errors and transient server responses.
        if [ "$attempt" -le "$RETRIES" ]; then
            case "$http_code" in
                000|429|500|502|503|504)
                    sleep "$attempt"
                    ;;
                *)
                    break
                    ;;
            esac
        fi
    done

    echo -e "${BOLD}▶ Testing Model: ${CYAN}${model}${NC}"
    echo -e "  • Provider : ${YELLOW}${pname}${NC}"
    echo -e "  • Endpoint : ${url}"

    if [ "$http_code" = "200" ]; then
        local content
        local reasoning

        content="$(python3 - "$tmp" <<'PY' 2>/dev/null
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
    value = d["choices"][0]["message"].get("content")
    print(value.replace("\n", " ") if value else "")
except Exception:
    pass
PY
)"

        reasoning="$(python3 - "$tmp" <<'PY' 2>/dev/null
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
    value = d["choices"][0]["message"].get("reasoning_content")
    print(value.replace("\n", " ") if value else "")
except Exception:
    pass
PY
)"

        if [ -n "$content" ]; then
            response_display="$content"

            echo -e "  • Status   : ${GREEN}200 OK${NC} (${elapsed}ms, attempt ${attempt})"
            echo -e "  • Response : \"${response_display:0:120}\""

            if [ "$content" = "ONLINE" ]; then
                echo -e "  • Behavior : ${GREEN}PASS — exact probe response${NC}"
            else
                WARN_COUNT=$((WARN_COUNT + 1))
                echo -e "  • Behavior : ${YELLOW}WARN — usable content, but probe was not exactly ONLINE${NC}"
            fi

            PASS_COUNT=$((PASS_COUNT + 1))

        elif [ -n "$reasoning" ]; then
            echo -e "  • Status   : ${RED}FAIL — reasoning-only response${NC} (${elapsed}ms, attempt ${attempt})"
            echo -e "  • Response : reasoning_content present but content is empty"
            FAIL_COUNT=$((FAIL_COUNT + 1))

        else
            echo -e "  • Status   : ${RED}FAIL — empty content response${NC} (${elapsed}ms, attempt ${attempt})"
            echo -e "  • Response : HTTP 200 but no usable content field"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        fi

        if [ "$elapsed" -ge 30000 ]; then
            SLOW_COUNT=$((SLOW_COUNT + 1))
            echo -e "  • Latency  : ${YELLOW}SLOW — ${elapsed}ms${NC}"
        fi

    else
        local error_msg

        error_msg="$(python3 - "$tmp" <<'PY' 2>/dev/null
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
    err = d.get("error", {})
    if isinstance(err, dict):
        print(err.get("message", "")[:180])
    else:
        print(str(err)[:180])
except Exception:
    pass
PY
)"

        if [ -z "$error_msg" ]; then
            if [ "$http_code" = "000" ]; then
                error_msg="Transport timeout/failure"
            else
                error_msg="HTTP ${http_code}"
            fi
        fi

        echo -e "  • Status   : ${RED}FAILED (HTTP ${http_code})${NC} (${elapsed}ms, attempts ${attempt})"
        echo -e "  • Error    : ${error_msg}"

        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    rm -f "$tmp" "${tmp}.status"
    echo
}

# --------------------------------------------------
# Run chat tests
# --------------------------------------------------

for MODEL in "${!ALL_MODELS[@]}"; do
    test_model "$MODEL"
done

# --------------------------------------------------
# Embedding test
# --------------------------------------------------

if [ -n "${NEXUS_EMBEDDING_URL:-}" ]; then
    EMBED_MODEL="${NEXUS_EMBEDDING_MODEL:-granite-311m-edge}"

    echo -e "${BOLD}▶ Testing Embedding: ${CYAN}${EMBED_MODEL}${NC}"
    echo -e "  • Provider : ${YELLOW}Lenovo Duet Edge Node${NC}"
    echo -e "  • Endpoint : ${EMBED_URL}"

    EMBED_TMP="$(mktemp)"

    START_MS="$(date +%s%3N)"

    curl -sS \
        --connect-timeout 5 \
        --max-time "$EMBED_TIMEOUT" \
        -o "$EMBED_TMP" \
        -w '%{http_code}' \
        -X POST "$EMBED_URL" \
        -H "Content-Type: application/json" \
        -d "{
            \"input\": \"AthenaFusionX substrate preflight check\",
            \"model\": \"${EMBED_MODEL}\"
        }" \
        > "${EMBED_TMP}.status" 2>/dev/null

    CURL_RC=$?
    END_MS="$(date +%s%3N)"
    EMBED_LATENCY=$((END_MS - START_MS))
    EMBED_CODE="$(cat "${EMBED_TMP}.status" 2>/dev/null || echo 000)"

    if [ "$CURL_RC" -eq 0 ] && [ "$EMBED_CODE" = "200" ]; then
        DIM="$(python3 - "$EMBED_TMP" <<'PY' 2>/dev/null
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
    print(len(d["data"][0]["embedding"]))
except Exception:
    print("unknown")
PY
)"

        echo -e "  • Status   : ${GREEN}200 OK${NC} (${EMBED_LATENCY}ms)"
        echo -e "  • Response : \"Verified ${DIM}-dim vector\""

        if [ "$DIM" = "768" ]; then
            echo -e "  • Contract : ${GREEN}PASS — expected 768 dimensions${NC}"
            PASS_COUNT=$((PASS_COUNT + 1))
        else
            echo -e "  • Contract : ${RED}FAIL — expected 768 dimensions, got ${DIM}${NC}"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        fi
    else
        echo -e "  • Status   : ${RED}FAILED (HTTP ${EMBED_CODE})${NC} (${EMBED_LATENCY}ms)"
        echo -e "  • Error    : Unable to generate embedding"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    rm -f "$EMBED_TMP" "${EMBED_TMP}.status"
    echo
fi

# --------------------------------------------------
# Verdict
# --------------------------------------------------

echo -e "${CYAN}=================================================="

if [ "$FAIL_COUNT" -eq 0 ]; then
    if [ "$WARN_COUNT" -gt 0 ]; then
        echo -e ">> PREFLIGHT VERDICT: ${YELLOW}READY — WARNINGS${NC}"
        echo -e "   ${PASS_COUNT}/${TOTAL_TESTS} passed; ${WARN_COUNT} warning(s); ${SLOW_COUNT} slow response(s)"
    elif [ "$SLOW_COUNT" -gt 0 ]; then
        echo -e ">> PREFLIGHT VERDICT: ${YELLOW}READY — SOME MODELS SLOW${NC}"
        echo -e "   ${PASS_COUNT}/${TOTAL_TESTS} passed; ${SLOW_COUNT} slow response(s)"
    else
        echo -e ">> PREFLIGHT VERDICT: ${GREEN}ALL CONFIGURED TARGETS READY${NC}"
        echo -e "   ${PASS_COUNT}/${TOTAL_TESTS} passed"
    fi
else
    echo -e ">> PREFLIGHT VERDICT: ${RED}DEGRADED${NC}"
    echo -e "   ${PASS_COUNT} passed, ${FAIL_COUNT} failed, ${WARN_COUNT} warning(s)"
fi

echo -e "==================================================${NC}"
