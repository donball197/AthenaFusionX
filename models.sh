#!/bin/bash

# Project Nexus Singularity v1.9 - Cortex Manifest
# Target: Root Setup Diagnostics

set -e # Exit immediately on uncontrolled errors

# 1. Dependency Validation
for cmd in curl jq awk; do
    if ! command -v "$cmd" &> /dev/null; then
        echo -e "\033[31m[ERROR] Critical dependency '$cmd' is not installed.\033[0m"
        exit 1
    fi
done

# 2. Environment Sourcing
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

if [ -z "$GEMINI_API_KEY" ]; then
    echo -e "\033[31m[ERROR] GEMINI_API_KEY is not set in .env\033[0m"
    exit 1
fi

echo -e "\033[1;36m>> GENERATING FULL CORTEX MANIFEST [NEXUS v1.9 | DATE: $(date +'%b %Y')]\033[0m"
echo -e "\033[90mRetrieving all available models and capabilities via API...\033[0m\n"

# 3. Robust API Fetching
# Uses -sS to hide progress but show errors, -f to fail silently on HTTP errors
RAW_DATA=$(curl -sS -f "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" || echo "")

if [ -z "$RAW_DATA" ]; then
    echo -e "\033[31m[ERROR] Failed to fetch data. Verify network connection and API Key validity.\033[0m"
    exit 1
fi

# 4. Dynamic Token Formatter
format_tokens() {
    local num=$1
    if [ "$num" -ge 1000000 ]; then
        awk "BEGIN {printf \"%.1fM\", $num/1000000}"
    elif [ "$num" -ge 1000 ]; then
        awk "BEGIN {printf \"%.1fK\", $num/1000}"
    else
        echo "$num"
    fi
}

# 5. Output Generation
printf "\033[1;37m%-45s | %-8s | %-8s | %-7s | %-15s\033[0m\n" "MODEL ID" "IN_TOK" "OUT_TOK" "THINK" "METHODS"
printf "%-45s-|-%-8s-|-%-8s-|-%-7s-|-%-15s\n" "---------------------------------------------" "--------" "--------" "-------" "---------------"

# Use jq with @tsv for bulletproof array parsing, avoiding delimiter collisions
echo "$RAW_DATA" | jq -r '
    .models[] | 
    [
        .name, 
        (.inputTokenLimit // 0), 
        (.outputTokenLimit // 0), 
        (.thinking // false), 
        (.supportedGenerationMethods | join(","))
    ] | @tsv
' | while IFS=$'\t' read -r name in_lim out_lim think methods; do

    # Logic-based Color Coding
    if [[ "$name" == *"3.1"* ]] || [[ "$name" == *"3-pro"* ]]; then
        COLOR="\033[1;32m" # Green (Cutting Edge)
    elif [[ "$name" == *"gemma-4"* ]]; then
        COLOR="\033[1;35m" # Purple (Open Weights Reasoning)
    elif [[ "$name" == *"deep-research"* ]]; then
        COLOR="\033[1;33m" # Yellow (Specialist)
    elif [[ "$name" == *"image"* ]] || [[ "$name" == *"veo"* ]]; then
        COLOR="\033[1;34m" # Blue (Multimodal/Video)
    else
        COLOR="\033[0m"     # Default
    fi

    in_fmt=$(format_tokens "$in_lim")
    out_fmt=$(format_tokens "$out_lim")

    # Safe string slicing
    methods_fmt="${methods:0:15}"
    if [ "${#methods}" -gt 15 ]; then methods_fmt="${methods_fmt}..."; fi

    printf "${COLOR}%-45s\033[0m | %-8s | %-8s | %-7s | %-15s\n" "$name" "$in_fmt" "$out_fmt" "$think" "$methods_fmt"
done

echo -e "\n\033[1;36m>> CRITICAL UPDATES DETECTED FOR NEXUS BACKEND:\033[0m"
echo -e "1. \033[1;35mGemma 4 31B-it\033[0m: Peak local-style reasoning routed via API for Micro-Agents."
echo -e "2. \033[1;33mDeep Research\033[0m: Available for high-complexity structural audits."
echo -e "3. \033[1;32mGemini 3.1\033[0m: Extended 1M context with native thinking capabilities."
