#!/bin/bash

# ================================================
# Project Nexus Singularity v3.1 - Enhanced Cortex Manifest
# Enhanced with better filtering, categorization, and recommendations
# ================================================

set -e

# Colors
RED='\033[31m'
GREEN='\033[32m'
YELLOW='\033[33m'
CYAN='\033[36m'
PURPLE='\033[35m'
BOLD='\033[1m'
RESET='\033[0m'

# 1. Dependency Check
for cmd in curl jq; do
    if ! command -v "$cmd" &> /dev/null; then
        echo -e "${RED}[ERROR] Critical dependency '$cmd' is missing.${RESET}"
        exit 1
    fi
done

# 2. API Key Check
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

if [ -z "$GEMINI_API_KEY" ]; then
    echo -e "${RED}[ERROR] GEMINI_API_KEY not found in .env${RESET}"
    exit 1
fi

echo -e "${BOLD}${CYAN}>> GENERATING ENHANCED CORTEX MANIFEST [NEXUS v3.1]${RESET}"
echo -e "${CYAN}Querying Google AI Studio for all active models...\n${RESET}"

# Fetch models
RAW_DATA=$(curl -sS -f "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" || echo "")

if [ -z "$RAW_DATA" ]; then
    echo -e "${RED}[ERROR] Failed to fetch models. Check network / API key.${RESET}"
    exit 1
fi

# Enhanced formatter
format_tokens() {
    local num=$1
    if [ "$num" -ge 1000000 ]; then
        echo "$((num / 1000000)).$(( (num % 1000000) / 100000 ))M"
    elif [ "$num" -ge 1000 ]; then
        echo "$((num / 1000)).$(( (num % 1000) / 100 ))K"
    else
        echo "$num"
    fi
}

# Header
printf "${BOLD}%-38s | %-10s | %-9s | %-9s | %-6s | %-22s${RESET}\n" \
    "MODEL ID" "VERSION" "INPUT" "OUTPUT" "TEMP" "CATEGORY"

printf "%-38s-|-%-10s-|-%-9s-|-%-9s-|-%-6s-|-%-22s\n" \
    "-------------------------------------" "----------" "---------" "---------" "------" "----------------------"

# Main enhanced parsing
echo "$RAW_DATA" | jq -r '
    .models[] | 
    select(.supportedGenerationMethods != null) |
    select(.supportedGenerationMethods | index("generateContent")) |
    {
        name: (.name | sub("^models/"; "")),
        version: (.version // "N/A"),
        input: (.inputTokenLimit // 0),
        output: (.outputTokenLimit // 0),
        temp: (.temperature // 0.0),
        display: (.displayName // "N/A")
    } | 
    [.name, .version, .input, .output, .temp, .display] | @tsv
' | while IFS=$'\t' read -r name version in_lim out_lim temp display; do

    # Category & Color
    if [[ "$name" == *"gemma-4-31b"* ]]; then
        COLOR="${PURPLE}"; CAT="GEMMA-4-HEAVY"
    elif [[ "$name" == *"gemma-4-26b"* ]]; then
        COLOR="${PURPLE}"; CAT="GEMMA-4-MOE"
    elif [[ "$name" == *"flash-lite"* ]]; then
        COLOR="${GREEN}"; CAT="FLASH-LITE"
    elif [[ "$name" == *"3.5-flash"* ]] || [[ "$name" == *"3.1-flash"* ]]; then
        COLOR="${GREEN}"; CAT="FLASH"
    elif [[ "$name" == *"pro"* ]] || [[ "$name" == *"deep-research"* ]]; then
        COLOR="${YELLOW}"; CAT="HEAVY/PRO"
    elif [[ "$name" == *"image"* ]] || [[ "$name" == *"vision"* ]]; then
        COLOR="${CYAN}"; CAT="MULTIMODAL"
    elif [[ "$name" == *"preview"* ]] || [[ "$name" == *"latest"* ]]; then
        COLOR="${CYAN}"; CAT="PREVIEW"
    else
        COLOR="${RESET}"; CAT="STANDARD"
    fi

    in_fmt=$(format_tokens "$in_lim")
    out_fmt=$(format_tokens "$out_lim")

    # Truncate display name
    if [ "${#display}" -gt 21 ]; then 
        disp_fmt="${display:0:18}..."
    else
        disp_fmt="$display"
    fi

    printf "${COLOR}%-38s${RESET} | %-10s | %-9s | %-9s | %-6s | %-22s\n" \
        "$name" "$version" "$in_fmt" "$out_fmt" "$temp" "$CAT"
done

# === RECOMMENDED CASCADES ===
echo -e "\n${BOLD}${CYAN}>> RECOMMENDED RUST CASCADES [v3.1]${RESET}"

cat << 'EOF'
pub fn get_light_models() -> Vec<String> {
    vec![
        "gemini-3.5-flash".to_string(),           // Tier 1 - Best balance
        "gemini-3.1-flash-lite".to_string(),      // Tier 2 - Fast & cheap
        "gemini-2.5-flash".to_string(),           // Tier 3 - Ultra stable
    ]
}

pub fn get_heavy_models() -> Vec<String> {
    vec![
        "nvidia/nemotron-3-ultra-550b-a55b".to_string(),             // Best open reasoning (262K)
        "gemini-3.1-pro-preview".to_string(),     // Frontier closed model
        "gemma-4-26b-a4b-it".to_string(),         // Efficient MoE fallback (262K)
    ]
}

pub fn get_flash_lite_models() -> Vec<String> {
    vec![
        "gemini-3.1-flash-lite".to_string(),      // Primary Flash Lite
        "gemini-3.1-flash-lite-preview".to_string(),
        "gemini-flash-lite-latest".to_string(),
    ]
}
EOF

echo -e "\n${GREEN}✅ Enhanced manifest generated successfully!${RESET}"
echo -e "${YELLOW}Pro Tip:${RESET} Gemma-4-31B is excellent for deep reasoning with 262K context."
